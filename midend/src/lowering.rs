// AST to IR lowering pass
// Converts semantic AST to SSA-based IR

use crate::builder::IRBuilder;
use crate::ir::{
    Function as IRFunction, Module as IRModule, Parameter, Terminator, Type as IRType, Value,
};
use spectra_compiler::ast::{
    BinaryOperator, Block, Enum as ASTEnum, EnumVariant, Expression, ExpressionKind, FStringPart,
    Function as ASTFunction, IfLetStatement, Item, Method as ASTMethod, Module as ASTModule,
    Statement, StatementKind, Struct as ASTStruct, TraitDeclaration, Type as ASTType,
    TypeAnnotation, TypeAnnotationKind, TypeParameter, UnaryOperator, Visibility,
    WhileLetStatement,
};
use spectra_compiler::error::MidendError;
use spectra_compiler::span::Span;
use std::collections::{HashMap, HashSet};

/// Stack-based scope system for variable shadowing support

#[derive(Debug, Clone)]
enum LoweredConstValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Char(char),
}

/// Maps a `BinaryOperator` to the method name used for operator overloading.
/// Returns `None` if the operator cannot be overloaded.
#[inline]
fn operator_overload_method(op: &BinaryOperator) -> Option<&'static str> {
    match op {
        BinaryOperator::Add => Some("add"),
        BinaryOperator::Subtract => Some("sub"),
        BinaryOperator::Multiply => Some("mul"),
        BinaryOperator::Divide => Some("div"),
        BinaryOperator::Modulo => Some("rem"),
        BinaryOperator::Equal | BinaryOperator::NotEqual => Some("eq"),
        BinaryOperator::Less => Some("lt"),
        BinaryOperator::LessEqual => Some("le"),
        BinaryOperator::Greater => Some("gt"),
        BinaryOperator::GreaterEqual => Some("ge"),
        BinaryOperator::And | BinaryOperator::Or => None,
    }
}

/// Stack-based scope system for variable shadowing support
#[derive(Clone)]
struct ScopeStack {
    scopes: Vec<HashMap<String, Value>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::with_capacity(16)],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::with_capacity(8));
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn insert(&mut self, name: String, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    fn get(&self, name: &str) -> Option<Value> {
        // Search from innermost to outermost scope
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(*value);
            }
        }
        None
    }

    fn clear(&mut self) {
        self.scopes.clear();
        self.scopes.push(HashMap::new());
    }
}

/// Scoped map that tracks IR types associated with variable names
#[derive(Clone)]
struct TypeScopeStack {
    scopes: Vec<HashMap<String, IRType>>,
}

impl TypeScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn insert(&mut self, name: String, ty: IRType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn get(&self, name: &str) -> Option<IRType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    fn clear(&mut self) {
        self.scopes.clear();
        self.scopes.push(HashMap::new());
    }
}

/// Metadata about an array lowered into IR
#[derive(Clone)]
struct ArrayInfo {
    ptr: Value,
    element_type: IRType,
    size: usize,
}

#[derive(Clone)]
struct ClosureCapture {
    name: String,
    ty: IRType,
}

#[derive(Clone)]
struct ClosureInfo {
    signature_params: Vec<IRType>,
    signature_return: IRType,
}

/// Scoped storage for array metadata (pointer, element type, size)
#[derive(Clone)]
struct ArrayScopeStack {
    scopes: Vec<HashMap<String, ArrayInfo>>,
}

impl ArrayScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn insert(&mut self, name: String, info: ArrayInfo) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, info);
        }
    }

    fn get(&self, name: &str) -> Option<&ArrayInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    fn clear(&mut self) {
        self.scopes.clear();
        self.scopes.push(HashMap::new());
    }
}

/// Scoped storage for struct pointers and their associated type names
#[derive(Clone)]
struct StructScopeStack {
    scopes: Vec<HashMap<String, (Value, String)>>,
}

impl StructScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn insert(&mut self, name: String, info: (Value, String)) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, info);
        }
    }

    fn get(&self, name: &str) -> Option<(Value, String)> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info.clone());
            }
        }
        None
    }

    fn clear(&mut self) {
        self.scopes.clear();
        self.scopes.push(HashMap::new());
    }
}

/// Loop context for break/continue handling
#[derive(Clone)]
struct LoopContext {
    header_block: usize,
    exit_block: usize,
}

#[derive(Clone)]
struct HostFunctionDescriptor {
    runtime_name: &'static str,
    return_type: IRType,
    returns_value: bool,
}

/// Represents a needed specialization of a generic function
#[derive(Debug, Clone)]
struct MonomorphizationRequest {
    /// Name of the generic function
    generic_name: String,
    /// Concrete types to substitute for type parameters (in order)
    concrete_types: Vec<IRType>,
}

impl MonomorphizationRequest {
    /// Generate mangled name for this specialization
    /// Example: process<Point> -> process_Point
    fn mangled_name(&self) -> String {
        let mut name = self.generic_name.clone();
        for ty in &self.concrete_types {
            name.push('_');
            name.push_str(&Self::type_to_string(ty));
        }
        name
    }

    fn type_to_string(ty: &IRType) -> String {
        match ty {
            IRType::Int => "int".to_string(),
            IRType::Float => "float".to_string(),
            IRType::Bool => "bool".to_string(),
            IRType::Pointer(inner) => format!("ptr_{}", Self::type_to_string(inner)),
            IRType::Struct { name, .. } => name.clone(),
            _ => "unknown".to_string(), // Fallback for other types
        }
    }
}

pub struct ASTLowering {
    builder: IRBuilder,
    current_function: Option<IRFunction>,
    value_map: ScopeStack,
    variable_types: TypeScopeStack,
    /// Maps variable names to their allocated memory locations (for mutable variables)
    alloca_map: HashMap<String, Value>,
    /// Maps array names to metadata for lowering (scoped)
    array_map: ArrayScopeStack,
    /// Maps struct names to their field definitions
    struct_definitions: HashMap<String, Vec<(String, IRType)>>,
    /// Maps struct variable names to (pointer, struct_name) for field access (scoped)
    struct_var_map: StructScopeStack,
    /// Maps enum names to their variant definitions: (variant_name, tag, data_types)
    enum_definitions: HashMap<String, Vec<(String, usize, Option<Vec<IRType>>)>>,
    /// Preserves declaration order for struct-style enum variant fields.
    enum_variant_field_names: HashMap<String, HashMap<String, Vec<String>>>,
    loop_stack: Vec<LoopContext>,
    /// Maps generic function names to their AST definitions (for monomorphization)
    generic_functions: HashMap<String, ASTFunction>,
    /// Maps generic struct names to their AST definitions
    generic_structs: HashMap<String, ASTStruct>,
    /// Maps generic enum names to their AST definitions
    generic_enums: HashMap<String, ASTEnum>,
    /// Requests for monomorphization that need to be processed
    pending_specializations: Vec<MonomorphizationRequest>,
    /// Already generated specializations (mangled_name -> IR function name)
    generated_specializations: HashMap<String, String>,
    /// Type substitution map for current monomorphization (type_param -> concrete_type)
    type_substitution_map: HashMap<String, IRType>,
    /// Maps (type_name, trait_name) -> true to track trait implementations
    trait_implementations: HashMap<(String, String), bool>,
    /// Tracks return types for lowered functions (including specializations)
    function_return_types: HashMap<String, IRType>,
    /// Maps bare imported names to their full stdlib path for unqualified call resolution.
    /// Populated from `Module::std_import_aliases` at the start of `lower_module()`.
    /// e.g. "print" → ["std", "io", "print"]
    std_import_aliases: HashMap<String, Vec<String>>,
    /// Counter for generating unique lambda function names.
    lambda_counter: usize,
    /// Lambdas collected during lowering that will be emitted as top-level IR functions.
    pending_lambdas: Vec<IRFunction>,
    /// Maps variable names that hold closures to their generated function and captured values.
    closure_var_map: HashMap<String, ClosureInfo>,
    /// Return type annotation of the function currently being lowered.
    /// Used to resolve Generic enum type args when they can't be fully inferred from
    /// the construction expression (e.g., Result::Ok(x) in a fn -> Result<int, string>).
    current_function_return_annotation: Option<TypeAnnotation>,
    /// Maps trait names to their methods in declaration order (for vtable slot lookup).
    trait_method_order: HashMap<String, Vec<String>>,
    /// Maps trait names to method signatures for dyn dispatch and type inference.
    trait_method_signatures: HashMap<String, HashMap<String, (Vec<IRType>, IRType)>>,
    /// Stores parsed trait declarations so default methods can be materialized for impls.
    trait_declarations: HashMap<String, TraitDeclaration>,
    /// Accumulated lowering errors that replace previous panics.
    errors: Vec<MidendError>,
    /// Compile-time constants lowered as literals at each use site.
    const_values: HashMap<String, LoweredConstValue>,
}

impl ASTLowering {
    pub fn new() -> Self {
        let mut lowering = Self {
            builder: IRBuilder::new(),
            current_function: None,
            value_map: ScopeStack::new(),
            variable_types: TypeScopeStack::new(),
            alloca_map: HashMap::new(),
            array_map: ArrayScopeStack::new(),
            struct_definitions: HashMap::new(),
            struct_var_map: StructScopeStack::new(),
            enum_definitions: HashMap::new(),
            enum_variant_field_names: HashMap::new(),
            loop_stack: Vec::new(),
            generic_functions: HashMap::new(),
            generic_structs: HashMap::new(),
            generic_enums: HashMap::new(),
            pending_specializations: Vec::new(),
            generated_specializations: HashMap::new(),
            type_substitution_map: HashMap::new(),
            trait_implementations: HashMap::new(),
            function_return_types: HashMap::new(),
            std_import_aliases: HashMap::new(),
            lambda_counter: 0,
            pending_lambdas: Vec::new(),
            closure_var_map: HashMap::new(),
            current_function_return_annotation: None,
            trait_method_order: HashMap::new(),
            trait_method_signatures: HashMap::new(),
            trait_declarations: HashMap::new(),
            errors: Vec::new(),
            const_values: HashMap::new(),
        };
        lowering.register_builtin_generic_enums();
        lowering
    }

    /// Pre-register `Option<T>` and `Result<T, E>` as built-in generic enums so
    /// that user code doesn't need to declare them.
    fn register_builtin_generic_enums(&mut self) {
        let dummy = Span::dummy();

        let make_type_param = |name: &str| TypeParameter {
            name: name.to_string(),
            bounds: vec![],
            span: dummy,
        };

        let simple_type_ann = |name: &str| TypeAnnotation {
            kind: TypeAnnotationKind::Simple {
                segments: vec![name.to_string()],
            },
            span: dummy,
        };

        // ── Option<T> ──────────────────────────────────────────────────────────
        let option_enum = ASTEnum {
            name: "Option".to_string(),
            span: dummy,
            visibility: Visibility::Public,
            type_params: vec![make_type_param("T")],
            variants: vec![
                EnumVariant {
                    name: "None".to_string(),
                    span: dummy,
                    data: None,
                    struct_data: None,
                },
                EnumVariant {
                    name: "Some".to_string(),
                    span: dummy,
                    data: Some(vec![simple_type_ann("T")]),
                    struct_data: None,
                },
            ],
        };
        self.generic_enums.insert("Option".to_string(), option_enum);

        // ── Result<T, E> ───────────────────────────────────────────────────────
        let result_enum = ASTEnum {
            name: "Result".to_string(),
            span: dummy,
            visibility: Visibility::Public,
            type_params: vec![make_type_param("T"), make_type_param("E")],
            variants: vec![
                EnumVariant {
                    name: "Ok".to_string(),
                    span: dummy,
                    data: Some(vec![simple_type_ann("T")]),
                    struct_data: None,
                },
                EnumVariant {
                    name: "Err".to_string(),
                    span: dummy,
                    data: Some(vec![simple_type_ann("E")]),
                    struct_data: None,
                },
            ],
        };
        self.generic_enums.insert("Result".to_string(), result_enum);
    }

    fn error(&mut self, message: impl Into<String>) {
        self.errors.push(MidendError::new(message));
    }

    fn eval_const_expression(&self, expr: &Expression) -> Option<LoweredConstValue> {
        match &expr.kind {
            ExpressionKind::NumberLiteral(raw) => {
                if raw.contains('.') {
                    raw.parse::<f64>().ok().map(LoweredConstValue::Float)
                } else {
                    raw.parse::<i64>().ok().map(LoweredConstValue::Int)
                }
            }
            ExpressionKind::StringLiteral(value) => Some(LoweredConstValue::String(value.clone())),
            ExpressionKind::BoolLiteral(value) => Some(LoweredConstValue::Bool(*value)),
            ExpressionKind::CharLiteral(value) => Some(LoweredConstValue::Char(*value)),
            ExpressionKind::Identifier(name) => self.const_values.get(name).cloned(),
            ExpressionKind::Grouping(inner) => self.eval_const_expression(inner),
            ExpressionKind::Unary { operator, operand } => {
                let value = self.eval_const_expression(operand)?;
                match (operator, value) {
                    (UnaryOperator::Negate, LoweredConstValue::Int(v)) => {
                        Some(LoweredConstValue::Int(-v))
                    }
                    (UnaryOperator::Negate, LoweredConstValue::Float(v)) => {
                        Some(LoweredConstValue::Float(-v))
                    }
                    (UnaryOperator::Not, LoweredConstValue::Bool(v)) => {
                        Some(LoweredConstValue::Bool(!v))
                    }
                    _ => None,
                }
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.eval_const_expression(left)?;
                let right = self.eval_const_expression(right)?;
                self.eval_const_binary(left, *operator, right)
            }
            ExpressionKind::Cast {
                expr: inner,
                target_type,
            } => {
                let value = self.eval_const_expression(inner)?;
                let target = self.lower_type_annotation(target_type);
                self.cast_const_value(value, &target)
            }
            _ => None,
        }
    }

    fn eval_const_binary(
        &self,
        left: LoweredConstValue,
        operator: BinaryOperator,
        right: LoweredConstValue,
    ) -> Option<LoweredConstValue> {
        use LoweredConstValue::*;

        match operator {
            BinaryOperator::Add => match (left, right) {
                (Int(a), Int(b)) => Some(Int(a + b)),
                (Float(a), Float(b)) => Some(Float(a + b)),
                (Int(a), Float(b)) => Some(Float(a as f64 + b)),
                (Float(a), Int(b)) => Some(Float(a + b as f64)),
                (String(a), String(b)) => Some(String(format!("{}{}", a, b))),
                _ => None,
            },
            BinaryOperator::Subtract => match (left, right) {
                (Int(a), Int(b)) => Some(Int(a - b)),
                (Float(a), Float(b)) => Some(Float(a - b)),
                (Int(a), Float(b)) => Some(Float(a as f64 - b)),
                (Float(a), Int(b)) => Some(Float(a - b as f64)),
                _ => None,
            },
            BinaryOperator::Multiply => match (left, right) {
                (Int(a), Int(b)) => Some(Int(a * b)),
                (Float(a), Float(b)) => Some(Float(a * b)),
                (Int(a), Float(b)) => Some(Float(a as f64 * b)),
                (Float(a), Int(b)) => Some(Float(a * b as f64)),
                _ => None,
            },
            BinaryOperator::Divide => match (left, right) {
                (Int(_), Int(0)) => None,
                (Int(a), Int(b)) => Some(Int(a / b)),
                (Float(a), Float(b)) if b != 0.0 => Some(Float(a / b)),
                (Int(a), Float(b)) if b != 0.0 => Some(Float(a as f64 / b)),
                (Float(a), Int(b)) if b != 0 => Some(Float(a / b as f64)),
                _ => None,
            },
            BinaryOperator::Modulo => match (left, right) {
                (Int(_), Int(0)) => None,
                (Int(a), Int(b)) => Some(Int(a % b)),
                _ => None,
            },
            BinaryOperator::Equal => Some(Bool(self.const_values_equal(&left, &right))),
            BinaryOperator::NotEqual => Some(Bool(!self.const_values_equal(&left, &right))),
            BinaryOperator::Less => self.eval_const_order(left, right, |a, b| a < b),
            BinaryOperator::LessEqual => self.eval_const_order(left, right, |a, b| a <= b),
            BinaryOperator::Greater => self.eval_const_order(left, right, |a, b| a > b),
            BinaryOperator::GreaterEqual => self.eval_const_order(left, right, |a, b| a >= b),
            BinaryOperator::And => match (left, right) {
                (Bool(a), Bool(b)) => Some(Bool(a && b)),
                _ => None,
            },
            BinaryOperator::Or => match (left, right) {
                (Bool(a), Bool(b)) => Some(Bool(a || b)),
                _ => None,
            },
        }
    }

    fn eval_const_order(
        &self,
        left: LoweredConstValue,
        right: LoweredConstValue,
        cmp: impl FnOnce(f64, f64) -> bool,
    ) -> Option<LoweredConstValue> {
        Some(LoweredConstValue::Bool(cmp(
            self.const_value_as_f64(&left)?,
            self.const_value_as_f64(&right)?,
        )))
    }

    fn const_value_as_f64(&self, value: &LoweredConstValue) -> Option<f64> {
        match value {
            LoweredConstValue::Int(v) => Some(*v as f64),
            LoweredConstValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    fn const_values_equal(&self, left: &LoweredConstValue, right: &LoweredConstValue) -> bool {
        match (left, right) {
            (LoweredConstValue::Int(a), LoweredConstValue::Int(b)) => a == b,
            (LoweredConstValue::Float(a), LoweredConstValue::Float(b)) => a == b,
            (LoweredConstValue::Int(a), LoweredConstValue::Float(b)) => (*a as f64) == *b,
            (LoweredConstValue::Float(a), LoweredConstValue::Int(b)) => *a == (*b as f64),
            (LoweredConstValue::Bool(a), LoweredConstValue::Bool(b)) => a == b,
            (LoweredConstValue::String(a), LoweredConstValue::String(b)) => a == b,
            (LoweredConstValue::Char(a), LoweredConstValue::Char(b)) => a == b,
            _ => false,
        }
    }

    fn cast_const_value(
        &self,
        value: LoweredConstValue,
        target: &IRType,
    ) -> Option<LoweredConstValue> {
        match (value, target) {
            (LoweredConstValue::Int(v), IRType::Int) => Some(LoweredConstValue::Int(v)),
            (LoweredConstValue::Int(v), IRType::Float) => Some(LoweredConstValue::Float(v as f64)),
            (LoweredConstValue::Int(v), IRType::Char) => {
                char::from_u32(v as u32).map(LoweredConstValue::Char)
            }
            (LoweredConstValue::Float(v), IRType::Float) => Some(LoweredConstValue::Float(v)),
            (LoweredConstValue::Float(v), IRType::Int) => Some(LoweredConstValue::Int(v as i64)),
            (LoweredConstValue::Char(v), IRType::Char) => Some(LoweredConstValue::Char(v)),
            (LoweredConstValue::Char(v), IRType::Int) => Some(LoweredConstValue::Int(v as i64)),
            (LoweredConstValue::Bool(v), IRType::Bool) => Some(LoweredConstValue::Bool(v)),
            (LoweredConstValue::String(v), IRType::String) => Some(LoweredConstValue::String(v)),
            _ => None,
        }
    }

    fn emit_const_value(&mut self, value: &LoweredConstValue, ir_func: &mut IRFunction) -> Value {
        match value {
            LoweredConstValue::Int(v) => self.builder.build_const_int(ir_func, *v),
            LoweredConstValue::Float(v) => self.builder.build_const_float(ir_func, *v),
            LoweredConstValue::Bool(v) => self.builder.build_const_bool(ir_func, *v),
            LoweredConstValue::String(v) => self.lower_string_literal(v, ir_func),
            LoweredConstValue::Char(v) => self.builder.build_const_int(ir_func, *v as i64),
        }
    }

    pub fn lower_module(&mut self, ast_module: &ASTModule) -> Result<IRModule, Vec<MidendError>> {
        let mut ir_module = IRModule::new(&ast_module.name);

        // Populate stdlib alias map for unqualified call resolution.
        self.std_import_aliases = ast_module.std_import_aliases.iter().cloned().collect();
        self.const_values.clear();
        for item in &ast_module.items {
            if let Item::Const(decl) = item {
                if let Some(value) = self.eval_const_expression(&decl.value) {
                    self.const_values.insert(decl.name.clone(), value);
                }
            }
        }

        // Pre-register return types of imported user functions so that
        // cross-module calls are not mis-classified as unknown closures by
        // the temporary bypass code in lower_expression.
        for (name, ty) in &ast_module.imported_function_return_types {
            let ir_ty = self.lower_type(ty);
            self.function_return_types
                .entry(name.clone())
                .or_insert(ir_ty);
        }

        // Pre-register enum/struct definitions from imported user modules so
        // that cross-module type references resolve before the local first pass.
        for enum_def in &ast_module.imported_enum_defs {
            if self.enum_definitions.contains_key(&enum_def.name) {
                // Check whether the existing registration is identical (same variant names).
                // Identical → safe duplicate (same type imported via two paths), skip.
                // Different → two modules export different types with the same name → error.
                let existing_variants: Vec<String> = self
                    .enum_definitions
                    .get(&enum_def.name)
                    .map(|v| v.iter().map(|(name, _, _)| name.clone()).collect())
                    .unwrap_or_default();
                let incoming_variants: Vec<String> =
                    enum_def.variants.iter().map(|v| v.name.clone()).collect();
                if existing_variants != incoming_variants {
                    self.error(format!(
                        "Name collision: two imported modules export different enum types named '{}'. \
                         Rename one of them to resolve the conflict.",
                        enum_def.name
                    ));
                }
                continue;
            }
            if !enum_def.type_params.is_empty() {
                self.generic_enums
                    .entry(enum_def.name.clone())
                    .or_insert_with(|| enum_def.clone());
            } else {
                let mut field_names = HashMap::new();
                let variants: Vec<(String, usize, Option<Vec<IRType>>)> = enum_def
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(tag, variant)| {
                        let data_types = if let Some(types) = variant.data.as_ref() {
                            Some(
                                types
                                    .iter()
                                    .map(|ty| self.lower_type_annotation(ty))
                                    .collect(),
                            )
                        } else if let Some(fields) = variant.struct_data.as_ref() {
                            field_names.insert(
                                variant.name.clone(),
                                fields.iter().map(|(name, _)| name.clone()).collect(),
                            );
                            Some(
                                fields
                                    .iter()
                                    .map(|(_, ty)| self.lower_type_annotation(ty))
                                    .collect(),
                            )
                        } else {
                            None
                        };
                        (variant.name.clone(), tag, data_types)
                    })
                    .collect();
                self.enum_definitions
                    .insert(enum_def.name.clone(), variants);
                if !field_names.is_empty() {
                    self.enum_variant_field_names
                        .insert(enum_def.name.clone(), field_names);
                }
            }
        }
        for struct_def in &ast_module.imported_struct_defs {
            if self.struct_definitions.contains_key(&struct_def.name) {
                // Check whether the existing registration is identical (same field names).
                let existing_fields: Vec<String> = self
                    .struct_definitions
                    .get(&struct_def.name)
                    .map(|v| v.iter().map(|(name, _)| name.clone()).collect())
                    .unwrap_or_default();
                let incoming_fields: Vec<String> =
                    struct_def.fields.iter().map(|f| f.name.clone()).collect();
                if existing_fields != incoming_fields {
                    self.error(format!(
                        "Name collision: two imported modules export different struct types named '{}'. \
                         Rename one of them to resolve the conflict.",
                        struct_def.name
                    ));
                }
                continue;
            }
            if !struct_def.type_params.is_empty() {
                self.generic_structs
                    .entry(struct_def.name.clone())
                    .or_insert_with(|| struct_def.clone());
            } else {
                let fields: Vec<(String, IRType)> = struct_def
                    .fields
                    .iter()
                    .map(|field| {
                        let field_type = self.lower_type_annotation(&field.ty);
                        (field.name.clone(), field_type)
                    })
                    .collect();
                self.struct_definitions
                    .insert(struct_def.name.clone(), fields);
            }
        }

        // First pass: collect struct and enum definitions, and trait implementations
        for item in &ast_module.items {
            if let Item::Struct(struct_def) = item {
                // Check if this is a generic struct
                if !struct_def.type_params.is_empty() {
                    // Store generic struct for later monomorphization
                    self.generic_structs
                        .insert(struct_def.name.clone(), struct_def.clone());
                    // generic struct stored for monomorphization
                } else {
                    // Regular struct - process immediately
                    let fields: Vec<(String, IRType)> = struct_def
                        .fields
                        .iter()
                        .map(|field| {
                            let field_type = self.lower_type_annotation(&field.ty);
                            (field.name.clone(), field_type)
                        })
                        .collect();
                    self.struct_definitions
                        .insert(struct_def.name.clone(), fields);
                }
            } else if let Item::Enum(enum_def) = item {
                // Check if this is a generic enum
                if !enum_def.type_params.is_empty() {
                    let shadows_builtin_generic =
                        matches!(enum_def.name.as_str(), "Option" | "Result");
                    // Store generic enum for later monomorphization
                    self.generic_enums
                        .insert(enum_def.name.clone(), enum_def.clone());
                    if shadows_builtin_generic {
                        self.enum_definitions.remove(&enum_def.name);
                    }
                    // generic enum stored for monomorphization
                } else {
                    // Regular enum - process immediately
                    let mut field_names = HashMap::new();
                    let variants: Vec<(String, usize, Option<Vec<IRType>>)> = enum_def
                        .variants
                        .iter()
                        .enumerate()
                        .map(|(tag, variant)| {
                            let data_types = if let Some(types) = variant.data.as_ref() {
                                Some(
                                    types
                                        .iter()
                                        .map(|ty| self.lower_type_annotation(ty))
                                        .collect(),
                                )
                            } else if let Some(fields) = variant.struct_data.as_ref() {
                                field_names.insert(
                                    variant.name.clone(),
                                    fields.iter().map(|(name, _)| name.clone()).collect(),
                                );
                                Some(
                                    fields
                                        .iter()
                                        .map(|(_, ty)| self.lower_type_annotation(ty))
                                        .collect(),
                                )
                            } else {
                                None
                            };
                            (variant.name.clone(), tag, data_types)
                        })
                        .collect();
                    self.enum_definitions
                        .insert(enum_def.name.clone(), variants);
                    if !field_names.is_empty() {
                        self.enum_variant_field_names
                            .insert(enum_def.name.clone(), field_names);
                    }
                }
            } else if let Item::Impl(impl_block) = item {
                // `impl Type { ... }` never has a trait_name (that goes to Item::TraitImpl).
                // Nothing to do here for trait registration in this path.
                let _ = impl_block;
            } else if let Item::TraitImpl(trait_impl) = item {
                // Register that `type_name` implements `trait_name`
                let key = (trait_impl.type_name.clone(), trait_impl.trait_name.clone());
                self.trait_implementations.insert(key, true);
            } else if let Item::Trait(trait_decl) = item {
                self.trait_declarations
                    .insert(trait_decl.name.clone(), trait_decl.clone());
                // Record method declaration order for vtable slot lookup
                let methods: Vec<String> =
                    trait_decl.methods.iter().map(|m| m.name.clone()).collect();
                self.trait_method_order
                    .insert(trait_decl.name.clone(), methods);
                let signatures = trait_decl
                    .methods
                    .iter()
                    .map(|method| {
                        let params = method
                            .params
                            .iter()
                            .filter(|param| !param.is_self)
                            .filter_map(|param| param.type_annotation.as_ref())
                            .map(|ann| self.lower_type_annotation(ann))
                            .collect::<Vec<_>>();
                        let ret = method
                            .return_type
                            .as_ref()
                            .map(|ann| self.lower_type_annotation(ann))
                            .unwrap_or(IRType::Void);
                        (method.name.clone(), (params, ret))
                    })
                    .collect::<HashMap<_, _>>();
                self.trait_method_signatures
                    .insert(trait_decl.name.clone(), signatures);
            }
        }

        // Second pass: pre-register return types for regular functions and impl methods
        for item in &ast_module.items {
            if let Item::Function(func) = item {
                if func.type_params.is_empty() {
                    let return_type = func
                        .return_type
                        .as_ref()
                        .map(|t| self.lower_type_annotation(t))
                        .unwrap_or(IRType::Void);
                    self.function_return_types
                        .insert(func.name.clone(), return_type);
                }
            } else if let Item::Impl(impl_block) = item {
                for method in &impl_block.methods {
                    let mangled = format!("{}_{}", impl_block.type_name, method.name);
                    let return_type = method
                        .return_type
                        .as_ref()
                        .map(|t| self.lower_type_annotation(t))
                        .unwrap_or(IRType::Void);
                    self.function_return_types
                        .entry(mangled)
                        .or_insert(return_type);
                }
            } else if let Item::TraitImpl(trait_impl) = item {
                for method in &trait_impl.methods {
                    let mangled = format!("{}_{}", trait_impl.type_name, method.name);
                    let return_type = method
                        .return_type
                        .as_ref()
                        .map(|t| self.lower_type_annotation(t))
                        .unwrap_or(IRType::Void);
                    self.function_return_types
                        .entry(mangled)
                        .or_insert(return_type);
                }
                for method in
                    self.collect_default_trait_methods(&trait_impl.trait_name, &trait_impl.methods)
                {
                    let mangled = format!("{}_{}", trait_impl.type_name, method.name);
                    let return_type = method
                        .return_type
                        .as_ref()
                        .map(|t| self.lower_type_annotation(t))
                        .unwrap_or(IRType::Void);
                    self.function_return_types
                        .entry(mangled)
                        .or_insert(return_type);
                }
            }
        }

        // Third pass: lower regular functions and impl block methods to IR
        for item in &ast_module.items {
            if let Item::Function(func) = item {
                // Store generic functions for later monomorphization
                if !func.type_params.is_empty() {
                    self.generic_functions
                        .insert(func.name.clone(), func.clone());
                    // generic function stored for monomorphization
                    continue;
                }

                let ir_func = self.lower_function(func);
                ir_module.add_function(ir_func);
            } else if let Item::Impl(impl_block) = item {
                for method in &impl_block.methods {
                    let ir_func = self.lower_method(method, &impl_block.type_name);
                    ir_module.add_function(ir_func);
                }
            } else if let Item::TraitImpl(trait_impl) = item {
                for method in &trait_impl.methods {
                    let ir_func = self.lower_method(method, &trait_impl.type_name);
                    ir_module.add_function(ir_func);
                }
                for method in
                    self.collect_default_trait_methods(&trait_impl.trait_name, &trait_impl.methods)
                {
                    let ir_func = self.lower_method(&method, &trait_impl.type_name);
                    ir_module.add_function(ir_func);
                }
            }
        }

        // Process pending monomorphization requests
        self.process_monomorphization_requests(&mut ir_module);

        // Emit any lambda functions collected during lowering
        let lambdas = std::mem::take(&mut self.pending_lambdas);
        for lambda_func in lambdas {
            ir_module.add_function(lambda_func);
        }

        if self.errors.is_empty() {
            Ok(ir_module)
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// Process all pending monomorphization requests
    fn process_monomorphization_requests(&mut self, ir_module: &mut IRModule) {
        // Safety limit: prevent infinite expansion from recursive/mutually-recursive
        // generics (e.g., Foo<T> → Foo<List<T>> → Foo<List<List<T>>> …).
        const MAX_SPECIALIZATIONS: usize = 512;
        let mut total_processed: usize = 0;

        // Process each pending specialization
        while let Some(request) = self.pending_specializations.pop() {
            if total_processed >= MAX_SPECIALIZATIONS {
                eprintln!(
                    "monomorphization limit ({}) reached for '{}'; remaining specializations skipped.",
                    MAX_SPECIALIZATIONS, request.generic_name
                );
                self.pending_specializations.clear();
                break;
            }

            let mangled = request.mangled_name();

            // Skip if already generated
            if self.generated_specializations.contains_key(&mangled) {
                continue;
            }

            // Get the generic function AST
            if let Some(generic_func) = self.generic_functions.get(&request.generic_name).cloned() {
                // generating specialization

                // Generate specialized function
                let specialized_func = self.specialize_function(&generic_func, &request);

                // Add to module
                ir_module.add_function(specialized_func.clone());

                // Mark as generated
                self.generated_specializations
                    .insert(mangled.clone(), specialized_func.name);

                total_processed += 1;
            } else {
                eprintln!(
                    "generic function '{}' not found for monomorphization",
                    request.generic_name
                );
            }
        }
    }

    /// Create a specialized version of a generic function
    fn specialize_function(
        &mut self,
        generic_func: &ASTFunction,
        request: &MonomorphizationRequest,
    ) -> IRFunction {
        // Create type substitution map: type_param_name -> IRType
        let mut type_map: HashMap<String, IRType> = HashMap::new();
        for (i, type_param) in generic_func.type_params.iter().enumerate() {
            if let Some(concrete_type) = request.concrete_types.get(i) {
                type_map.insert(type_param.name.clone(), concrete_type.clone());
            }
        }

        // Validate trait bounds
        for (i, type_param) in generic_func.type_params.iter().enumerate() {
            if let Some(concrete_type) = request.concrete_types.get(i) {
                // Check each trait bound
                for bound in &type_param.bounds {
                    if !self.type_satisfies_trait(concrete_type, bound) {
                        let type_name = self.ir_type_to_ast_name(concrete_type);
                        self.error(format!(
                            "Trait bound violation: Type '{}' does not implement trait '{}' required by type parameter '{}'. \
                             Function '{}' requires {} to have trait {}. \
                             Specialization: {}",
                            type_name, bound, type_param.name,
                            generic_func.name, type_param.name, bound,
                            request.mangled_name()
                        ));
                    }
                }
            }
        }

        let mangled_name = request.mangled_name();

        // Create specialized function by copying generic and renaming
        let mut specialized = generic_func.clone();
        specialized.name = mangled_name.clone();
        specialized.type_params.clear(); // Remove generic parameters

        // Substitute type parameters in function signature
        for param in &mut specialized.params {
            if let Some(ref mut ty) = param.ty {
                self.substitute_type_in_annotation(ty, &type_map);
            }
        }

        if let Some(ref mut return_ty) = specialized.return_type {
            self.substitute_type_in_annotation(return_ty, &type_map);
        }

        // Set type substitution map for lowering
        self.type_substitution_map = type_map;

        // Lower the specialized function
        let result = self.lower_function(&specialized);

        // Clear type substitution map after lowering
        self.type_substitution_map.clear();

        result
    }

    /// Substitute type parameters in a TypeAnnotation
    fn substitute_type_in_annotation(
        &self,
        annotation: &mut TypeAnnotation,
        type_map: &HashMap<String, IRType>,
    ) {
        use spectra_compiler::ast::TypeAnnotationKind;

        match &mut annotation.kind {
            TypeAnnotationKind::Simple { segments } => {
                // Check if this is a type parameter (e.g., "T")
                if segments.len() == 1 {
                    let name = &segments[0];
                    if let Some(concrete_type) = type_map.get(name) {
                        // Replace with concrete type name
                        let concrete_name = self.ir_type_to_ast_name(concrete_type);
                        segments[0] = concrete_name;
                    }
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                // Recursively substitute in tuple elements
                for elem in elements {
                    self.substitute_type_in_annotation(elem, type_map);
                }
            }
            TypeAnnotationKind::Function {
                params,
                return_type,
            } => {
                for param in params {
                    self.substitute_type_in_annotation(param, type_map);
                }
                self.substitute_type_in_annotation(return_type, type_map);
            }
            TypeAnnotationKind::Generic { name: _, type_args } => {
                for arg in type_args {
                    self.substitute_type_in_annotation(arg, type_map);
                }
            }
            TypeAnnotationKind::DynTrait { .. } => {}
        }
    }

    /// Convert IRType to AST type name for substitution
    fn ir_type_to_ast_name(&self, ty: &IRType) -> String {
        match ty {
            IRType::Int => "int".to_string(),
            IRType::Float => "float".to_string(),
            IRType::Bool => "bool".to_string(),
            IRType::String => "string".to_string(),
            IRType::Char => "char".to_string(),
            IRType::Struct { name, .. } => name.clone(),
            IRType::Pointer(inner) => format!("ptr<{}>", self.ir_type_to_ast_name(inner)),
            _ => "unknown".to_string(),
        }
    }

    /// Check if a concrete type satisfies a trait bound
    fn type_satisfies_trait(&self, concrete_type: &IRType, trait_name: &str) -> bool {
        let type_name = self.ir_type_to_ast_name(concrete_type);

        // Check if we have recorded this implementation
        let key = (type_name, trait_name.to_string());
        self.trait_implementations
            .get(&key)
            .copied()
            .unwrap_or(false)
    }

    fn merge_array_element_types(&self, left: &IRType, right: &IRType) -> Option<IRType> {
        if left == right {
            return Some(left.clone());
        }

        match (left, right) {
            (IRType::Int, IRType::Float) | (IRType::Float, IRType::Int) => Some(IRType::Float),
            (IRType::Pointer(l), IRType::Pointer(r)) => self
                .merge_array_element_types(l.as_ref(), r.as_ref())
                .map(|merged| IRType::Pointer(Box::new(merged))),
            (
                IRType::Array {
                    element_type: l_elem,
                    size: l_size,
                },
                IRType::Array {
                    element_type: r_elem,
                    size: r_size,
                },
            ) => {
                if l_size != r_size {
                    None
                } else {
                    self.merge_array_element_types(l_elem.as_ref(), r_elem.as_ref())
                        .map(|merged| IRType::Array {
                            element_type: Box::new(merged),
                            size: *l_size,
                        })
                }
            }
            (
                IRType::Struct {
                    name: l_name,
                    fields: l_fields,
                },
                IRType::Struct {
                    name: r_name,
                    fields: r_fields,
                },
            ) => {
                if l_name == r_name && l_fields == r_fields {
                    Some(IRType::Struct {
                        name: l_name.clone(),
                        fields: l_fields.clone(),
                    })
                } else {
                    None
                }
            }
            (
                IRType::Enum {
                    name: l_name,
                    variants: l_variants,
                },
                IRType::Enum {
                    name: r_name,
                    variants: r_variants,
                },
            ) => {
                if l_name == r_name && l_variants == r_variants {
                    Some(IRType::Enum {
                        name: l_name.clone(),
                        variants: l_variants.clone(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn infer_array_element_type(&self, elements: &[Expression]) -> IRType {
        if elements.is_empty() {
            return IRType::Int;
        }

        let mut element_type = self.infer_expr_ir_type(&elements[0]);

        for expr in elements.iter().skip(1) {
            let next_type = self.infer_expr_ir_type(expr);
            match self.merge_array_element_types(&element_type, &next_type) {
                Some(merged) => {
                    element_type = merged;
                }
                None => {
                    element_type = IRType::Int;
                    break;
                }
            }
        }

        element_type
    }

    fn ensure_struct_definition(
        &mut self,
        base_name: &str,
        type_args: &[TypeAnnotation],
    ) -> (String, Vec<(String, IRType)>) {
        if type_args.is_empty() {
            if let Some(fields) = self.struct_definitions.get(base_name).cloned() {
                return (base_name.to_string(), fields);
            }

            if let Some(generic_struct) = self.generic_structs.get(base_name).cloned() {
                let fallback_args: Vec<TypeAnnotation> = generic_struct
                    .type_params
                    .iter()
                    .map(|_| Self::unknown_type_annotation())
                    .collect();
                return self.ensure_struct_definition(base_name, &fallback_args);
            }

            self.error(format!(
                "Struct '{}' was not registered before lowering; check the semantic phase",
                base_name
            ));
            return (base_name.to_string(), Vec::new());
        }

        let type_names: Vec<String> = type_args
            .iter()
            .map(|ty| self.type_annotation_to_string(ty))
            .collect();
        let mangled = format!("{}_{}", base_name, type_names.join("_"));

        if !self.struct_definitions.contains_key(&mangled) {
            if let Some(generic_struct) = self.generic_structs.get(base_name).cloned() {
                self.specialize_struct(&generic_struct, type_args, &mangled);
            } else {
                self.error(format!(
                    "Generic struct '{}' not found for specialization with arguments {:?}",
                    base_name, type_names
                ));
            }
        }

        let fields = self
            .struct_definitions
            .get(&mangled)
            .cloned()
            .unwrap_or_else(|| {
                self.error(format!(
                    "Struct '{}' not registered after specialization",
                    mangled
                ));
                Vec::new()
            });

        (mangled, fields)
    }

    fn ensure_enum_definition(
        &mut self,
        base_name: &str,
        type_args: &[TypeAnnotation],
    ) -> (String, Vec<(String, usize, Option<Vec<IRType>>)>) {
        if type_args.is_empty() {
            if let Some(variants) = self.enum_definitions.get(base_name).cloned() {
                return (base_name.to_string(), variants);
            }

            if let Some(generic_enum) = self.generic_enums.get(base_name).cloned() {
                let fallback_args: Vec<TypeAnnotation> = generic_enum
                    .type_params
                    .iter()
                    .map(|_| Self::unknown_type_annotation())
                    .collect();
                return self.ensure_enum_definition(base_name, &fallback_args);
            }

            self.error(format!(
                "Enum '{}' was not registered before lowering; check the semantic phase",
                base_name
            ));
            return (base_name.to_string(), Vec::new());
        }

        let type_names: Vec<String> = type_args
            .iter()
            .map(|ty| self.type_annotation_to_string(ty))
            .collect();
        let mangled = format!("{}_{}", base_name, type_names.join("_"));

        if !self.enum_definitions.contains_key(&mangled) {
            if let Some(generic_enum) = self.generic_enums.get(base_name).cloned() {
                self.specialize_enum(&generic_enum, type_args, &mangled);
            } else {
                self.error(format!(
                    "Generic enum '{}' not found for specialization with arguments {:?}",
                    base_name, type_names
                ));
            }
        }

        let variants = self
            .enum_definitions
            .get(&mangled)
            .cloned()
            .unwrap_or_else(|| {
                self.error(format!(
                    "Enum '{}' not registered after specialization",
                    mangled
                ));
                Vec::new()
            });

        (mangled, variants)
    }

    fn resolve_struct_type(&self, base_name: &str, type_args: &[TypeAnnotation]) -> Option<IRType> {
        if type_args.is_empty() {
            return self
                .struct_definitions
                .get(base_name)
                .cloned()
                .map(|fields| IRType::Struct {
                    name: base_name.to_string(),
                    fields,
                });
        }

        let type_names: Vec<String> = type_args
            .iter()
            .map(|ty| self.type_annotation_to_string(ty))
            .collect();
        let mangled = format!("{}_{}", base_name, type_names.join("_"));

        if let Some(fields) = self.struct_definitions.get(&mangled) {
            return Some(IRType::Struct {
                name: mangled,
                fields: fields.clone(),
            });
        }

        if let Some(generic_struct) = self.generic_structs.get(base_name) {
            if generic_struct.type_params.len() != type_args.len() {
                return None;
            }

            let mut type_map: HashMap<String, TypeAnnotation> = HashMap::new();
            for (param, arg) in generic_struct.type_params.iter().zip(type_args.iter()) {
                type_map.insert(param.name.clone(), arg.clone());
            }

            let fields: Vec<(String, IRType)> = generic_struct
                .fields
                .iter()
                .map(|field| {
                    let substituted = self.substitute_type(&field.ty, &type_map);
                    let ir_type = self.lower_type_annotation(&substituted);
                    (field.name.clone(), ir_type)
                })
                .collect();

            return Some(IRType::Struct {
                name: mangled,
                fields,
            });
        }

        None
    }

    fn resolve_enum_type(&self, base_name: &str, type_args: &[TypeAnnotation]) -> Option<IRType> {
        let mut enum_name = base_name.to_string();
        let variants_data = if type_args.is_empty() {
            self.enum_definitions.get(base_name).cloned()
        } else {
            let type_names: Vec<String> = type_args
                .iter()
                .map(|ty| self.type_annotation_to_string(ty))
                .collect();
            let mangled = format!("{}_{}", base_name, type_names.join("_"));
            enum_name = mangled.clone();

            let mut entry = self.enum_definitions.get(&mangled).cloned();
            if entry.is_none() {
                if let Some(generic_enum) = self.generic_enums.get(base_name) {
                    if generic_enum.type_params.len() != type_args.len() {
                        return None;
                    }

                    let mut type_map: HashMap<String, TypeAnnotation> = HashMap::new();
                    for (param, arg) in generic_enum.type_params.iter().zip(type_args.iter()) {
                        type_map.insert(param.name.clone(), arg.clone());
                    }

                    let computed: Vec<(String, usize, Option<Vec<IRType>>)> = generic_enum
                        .variants
                        .iter()
                        .enumerate()
                        .map(|(tag, variant)| {
                            let data_types = variant.data.as_ref().map(|types| {
                                types
                                    .iter()
                                    .map(|ty| {
                                        let substituted = self.substitute_type(ty, &type_map);
                                        self.lower_type_annotation(&substituted)
                                    })
                                    .collect::<Vec<_>>()
                            });
                            (variant.name.clone(), tag, data_types)
                        })
                        .collect();

                    entry = Some(computed);
                }
            }

            entry
        };

        variants_data.map(|variants| {
            let simplified: Vec<(String, Option<Vec<IRType>>)> = variants
                .into_iter()
                .map(|(name, _, data)| (name, data))
                .collect();

            IRType::Enum {
                name: enum_name,
                variants: simplified,
            }
        })
    }

    fn infer_block_result_type(&self, block: &Block) -> Option<IRType> {
        let mut result: Option<IRType> = None;

        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Return(ret) => {
                    let ty = ret
                        .value
                        .as_ref()
                        .map(|expr| self.infer_expr_ir_type(expr))
                        .unwrap_or(IRType::Void);
                    return Some(ty);
                }
                StatementKind::Expression(expr) => {
                    result = Some(self.infer_expr_ir_type(expr));
                }
                _ => {}
            }
        }

        result
    }

    fn unknown_type_annotation() -> TypeAnnotation {
        TypeAnnotation {
            kind: TypeAnnotationKind::Simple {
                segments: vec!["unknown".to_string()],
            },
            span: Span::dummy(),
        }
    }

    fn simple_type_annotation(name: &str) -> TypeAnnotation {
        TypeAnnotation {
            kind: TypeAnnotationKind::Simple {
                segments: vec![name.to_string()],
            },
            span: Span::dummy(),
        }
    }

    fn is_unknown_annotation(type_ann: &TypeAnnotation) -> bool {
        matches!(
            &type_ann.kind,
            TypeAnnotationKind::Simple { segments }
                if segments.len() == 1 && segments[0] == "unknown"
        )
    }

    fn ir_type_to_annotation(&self, ir_type: &IRType) -> TypeAnnotation {
        match ir_type {
            IRType::Int => Self::simple_type_annotation("int"),
            IRType::Float => Self::simple_type_annotation("float"),
            IRType::Bool => Self::simple_type_annotation("bool"),
            IRType::String => Self::simple_type_annotation("string"),
            IRType::Char => Self::simple_type_annotation("char"),
            IRType::Struct { name, .. } => self
                .specialized_generic_annotation(name)
                .unwrap_or_else(|| Self::simple_type_annotation(name)),
            IRType::Enum { name, variants } => self
                .specialized_generic_annotation_from_enum(name, variants)
                .or_else(|| self.specialized_generic_annotation(name))
                .unwrap_or_else(|| Self::simple_type_annotation(name)),
            IRType::Array { element_type, .. } => {
                // Represent arrays by their element type (best effort)
                self.ir_type_to_annotation(element_type.as_ref())
            }
            IRType::Tuple { elements } => TypeAnnotation {
                kind: TypeAnnotationKind::Tuple {
                    elements: elements
                        .iter()
                        .map(|elem| self.ir_type_to_annotation(elem))
                        .collect(),
                },
                span: Span::dummy(),
            },
            IRType::Pointer(inner) => self.ir_type_to_annotation(inner.as_ref()),
            IRType::Void => Self::simple_type_annotation("void"),
            _ => Self::unknown_type_annotation(),
        }
    }

    fn specialized_generic_annotation(&self, type_name: &str) -> Option<TypeAnnotation> {
        self.generic_enums
            .iter()
            .find_map(|(base_name, generic_enum)| {
                let prefix = format!("{}_", base_name);
                if !type_name.starts_with(&prefix) {
                    return None;
                }

                let suffix = &type_name[prefix.len()..];
                let parts: Vec<&str> = suffix.split('_').collect();
                if parts.len() != generic_enum.type_params.len() {
                    return None;
                }

                let type_args = parts
                    .into_iter()
                    .map(Self::simple_type_annotation)
                    .collect();

                Some(TypeAnnotation {
                    kind: TypeAnnotationKind::Generic {
                        name: base_name.clone(),
                        type_args,
                    },
                    span: Span::dummy(),
                })
            })
            .or_else(|| {
                self.generic_structs
                    .iter()
                    .find_map(|(base_name, generic_struct)| {
                        let prefix = format!("{}_", base_name);
                        if !type_name.starts_with(&prefix) {
                            return None;
                        }

                        let suffix = &type_name[prefix.len()..];
                        let parts: Vec<&str> = suffix.split('_').collect();
                        if parts.len() != generic_struct.type_params.len() {
                            return None;
                        }

                        let type_args = parts
                            .into_iter()
                            .map(Self::simple_type_annotation)
                            .collect();

                        Some(TypeAnnotation {
                            kind: TypeAnnotationKind::Generic {
                                name: base_name.clone(),
                                type_args,
                            },
                            span: Span::dummy(),
                        })
                    })
            })
    }

    fn specialized_generic_annotation_from_enum(
        &self,
        type_name: &str,
        variants: &[(String, Option<Vec<IRType>>)],
    ) -> Option<TypeAnnotation> {
        for (base_name, generic_enum) in &self.generic_enums {
            if generic_enum.variants.len() != variants.len() {
                continue;
            }

            let mut param_positions: HashMap<String, usize> = HashMap::new();
            for (idx, param) in generic_enum.type_params.iter().enumerate() {
                param_positions.insert(param.name.clone(), idx);
            }

            let mut inferred =
                vec![Self::unknown_type_annotation(); generic_enum.type_params.len()];
            let mut matches_shape = true;

            for (template_variant, actual_variant) in
                generic_enum.variants.iter().zip(variants.iter())
            {
                let (actual_name, actual_data) = actual_variant;
                if template_variant.name != *actual_name {
                    matches_shape = false;
                    break;
                }

                match (&template_variant.data, actual_data) {
                    (Some(template_types), Some(actual_types)) => {
                        if template_types.len() != actual_types.len() {
                            matches_shape = false;
                            break;
                        }

                        for (template, actual_type) in
                            template_types.iter().zip(actual_types.iter())
                        {
                            self.fill_type_args_from_annotation(
                                template,
                                actual_type,
                                &param_positions,
                                &mut inferred,
                            );
                        }
                    }
                    (None, None) => {}
                    _ => {
                        matches_shape = false;
                        break;
                    }
                }
            }

            if !matches_shape || inferred.iter().any(Self::is_unknown_annotation) {
                continue;
            }

            return Some(TypeAnnotation {
                kind: TypeAnnotationKind::Generic {
                    name: base_name.clone(),
                    type_args: inferred,
                },
                span: Span::dummy(),
            });
        }

        self.specialized_generic_annotation(type_name)
    }

    fn default_type_args_for_enum(&self, enum_name: &str) -> Option<Vec<TypeAnnotation>> {
        self.generic_enums.get(enum_name).map(|generic_enum| {
            generic_enum
                .type_params
                .iter()
                .map(|_| Self::unknown_type_annotation())
                .collect()
        })
    }

    fn fill_type_args_from_annotation(
        &self,
        template: &TypeAnnotation,
        actual_type: &IRType,
        param_positions: &HashMap<String, usize>,
        inferred: &mut [TypeAnnotation],
    ) {
        match &template.kind {
            TypeAnnotationKind::Simple { segments } if segments.len() == 1 => {
                if let Some(&index) = param_positions.get(&segments[0]) {
                    if Self::is_unknown_annotation(&inferred[index]) {
                        inferred[index] = self.ir_type_to_annotation(actual_type);
                    }
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                if let IRType::Tuple {
                    elements: actual_elements,
                } = actual_type
                {
                    for (sub_template, sub_type) in elements.iter().zip(actual_elements.iter()) {
                        self.fill_type_args_from_annotation(
                            sub_template,
                            sub_type,
                            param_positions,
                            inferred,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn type_annotation_needs_refinement(&self, ann: &TypeAnnotation) -> bool {
        match &ann.kind {
            TypeAnnotationKind::Simple { segments } if segments.len() == 1 => {
                let name = &segments[0];
                if name == "unknown" {
                    true
                } else if self.enum_definitions.contains_key(name)
                    || self.struct_definitions.contains_key(name)
                {
                    false
                } else if self.generic_enums.contains_key(name) {
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn infer_enum_type_args_from_data(
        &self,
        enum_name: &str,
        variant_name: &str,
        data_exprs: &[Expression],
    ) -> Option<Vec<TypeAnnotation>> {
        let (param_names, field_templates) = {
            let generic_enum = self.generic_enums.get(enum_name)?;
            let variant = generic_enum
                .variants
                .iter()
                .find(|v| v.name == variant_name)?;
            let data = variant.data.as_ref()?;
            let params = generic_enum
                .type_params
                .iter()
                .map(|param| param.name.clone())
                .collect::<Vec<_>>();
            (params, data.clone())
        };

        if field_templates.len() != data_exprs.len() {
            return None;
        }

        let mut param_positions: HashMap<String, usize> = HashMap::new();
        for (idx, param_name) in param_names.iter().enumerate() {
            param_positions.insert(param_name.clone(), idx);
        }

        let mut inferred = vec![Self::unknown_type_annotation(); param_names.len()];

        for (template, expr) in field_templates.iter().zip(data_exprs.iter()) {
            if let TypeAnnotationKind::Simple { segments } = &template.kind {
                if segments.len() == 1 {
                    if let Some(&index) = param_positions.get(&segments[0]) {
                        if Self::is_unknown_annotation(&inferred[index]) {
                            if let Some(annotation) = self.infer_expr_type_annotation(expr) {
                                inferred[index] = annotation;
                                continue;
                            }
                        }
                    }
                }
            }

            let actual_type = self.infer_expr_ir_type(expr);
            self.fill_type_args_from_annotation(
                template,
                &actual_type,
                &param_positions,
                &mut inferred,
            );
        }

        Some(inferred)
    }

    fn infer_expr_type_annotation(&self, expr: &Expression) -> Option<TypeAnnotation> {
        match &expr.kind {
            ExpressionKind::NumberLiteral(num) => {
                Some(Self::simple_type_annotation(if num.contains('.') {
                    "float"
                } else {
                    "int"
                }))
            }
            ExpressionKind::StringLiteral(_) => Some(Self::simple_type_annotation("string")),
            ExpressionKind::BoolLiteral(_) => Some(Self::simple_type_annotation("bool")),
            ExpressionKind::StructLiteral {
                name, type_args, ..
            } => {
                if type_args.is_empty() {
                    Some(Self::simple_type_annotation(name))
                } else {
                    Some(TypeAnnotation {
                        kind: TypeAnnotationKind::Generic {
                            name: name.clone(),
                            type_args: type_args.clone(),
                        },
                        span: Span::dummy(),
                    })
                }
            }
            ExpressionKind::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
                ..
            } => {
                let needs_refinement = type_args.is_empty()
                    || type_args
                        .iter()
                        .any(|ann| self.type_annotation_needs_refinement(ann));

                let final_args = if needs_refinement {
                    if let Some(data_exprs) = data {
                        self.infer_enum_type_args_from_data(enum_name, variant_name, data_exprs)
                            .or_else(|| self.default_type_args_for_enum(enum_name))
                            .unwrap_or_default()
                    } else if let Some(named_fields) = struct_data {
                        self.infer_enum_type_args_from_named_fields(
                            enum_name,
                            variant_name,
                            named_fields,
                        )
                        .or_else(|| self.default_type_args_for_enum(enum_name))
                        .unwrap_or_default()
                    } else {
                        self.default_type_args_for_enum(enum_name)
                            .unwrap_or_default()
                    }
                } else {
                    type_args.clone()
                };

                if final_args.is_empty() {
                    Some(Self::simple_type_annotation(enum_name))
                } else {
                    Some(TypeAnnotation {
                        kind: TypeAnnotationKind::Generic {
                            name: enum_name.clone(),
                            type_args: final_args,
                        },
                        span: Span::dummy(),
                    })
                }
            }
            _ => None,
        }
    }

    fn infer_enum_type_args_from_named_fields(
        &self,
        enum_name: &str,
        variant_name: &str,
        fields: &[(String, Expression)],
    ) -> Option<Vec<TypeAnnotation>> {
        let generic_enum = self.generic_enums.get(enum_name)?;
        let variant = generic_enum
            .variants
            .iter()
            .find(|v| v.name == variant_name)?;
        let field_templates = variant.struct_data.as_ref()?;

        let ordered_exprs: Vec<&Expression> = field_templates
            .iter()
            .map(|(field_name, _)| {
                fields
                    .iter()
                    .find(|(name, _)| name == field_name)
                    .map(|(_, expr)| expr)
            })
            .collect::<Option<Vec<_>>>()?;

        let param_names = generic_enum
            .type_params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>();

        let mut param_positions: HashMap<String, usize> = HashMap::new();
        for (idx, param_name) in param_names.iter().enumerate() {
            param_positions.insert(param_name.clone(), idx);
        }

        let mut inferred = vec![Self::unknown_type_annotation(); param_names.len()];

        for ((_, template), expr) in field_templates.iter().zip(ordered_exprs.iter()) {
            let actual_type = self.infer_expr_ir_type(expr);
            self.fill_type_args_from_annotation(
                template,
                &actual_type,
                &param_positions,
                &mut inferred,
            );
        }

        Some(inferred)
    }

    fn reorder_named_variant_exprs<'a>(
        &self,
        enum_name: &str,
        variant_name: &str,
        fields: &'a [(String, Expression)],
    ) -> Option<Vec<&'a Expression>> {
        let order = self
            .enum_variant_field_names
            .get(enum_name)?
            .get(variant_name)?;

        order
            .iter()
            .map(|field_name| {
                fields
                    .iter()
                    .find(|(name, _)| name == field_name)
                    .map(|(_, expr)| expr)
            })
            .collect()
    }

    fn reorder_named_variant_patterns<'a>(
        &self,
        enum_name: &str,
        variant_name: &str,
        fields: &'a [(String, spectra_compiler::ast::Pattern)],
    ) -> Option<Vec<&'a spectra_compiler::ast::Pattern>> {
        let order = self
            .enum_variant_field_names
            .get(enum_name)?
            .get(variant_name)?;

        order
            .iter()
            .map(|field_name| {
                fields
                    .iter()
                    .find(|(name, _)| name == field_name)
                    .map(|(_, pattern)| pattern)
            })
            .collect()
    }

    fn enum_variants_from_ir_type(
        &self,
        scrutinee_type: Option<&IRType>,
    ) -> Option<Vec<(String, usize, Option<Vec<IRType>>)>> {
        if let Some(IRType::Enum { variants, .. }) = scrutinee_type {
            return Some(
                variants
                    .iter()
                    .enumerate()
                    .map(|(tag, (name, data))| (name.clone(), tag, data.clone()))
                    .collect(),
            );
        }
        None
    }

    fn merge_types(&self, left: &IRType, right: &IRType) -> Option<IRType> {
        if left == right {
            return Some(left.clone());
        }

        match (left, right) {
            (IRType::Int, IRType::Float) | (IRType::Float, IRType::Int) => Some(IRType::Float),
            (IRType::Void, other) => Some(other.clone()),
            (other, IRType::Void) => Some(other.clone()),
            _ => None,
        }
    }

    fn unify_types(&self, mut types: Vec<IRType>) -> IRType {
        if types.is_empty() {
            return IRType::Void;
        }

        let mut result = types.remove(0);
        for ty in types {
            if let Some(merged) = self.merge_types(&result, &ty) {
                result = merged;
            }
        }

        result
    }

    /// Infere o tipo IR de uma expressão AST (análise simplificada)
    fn infer_expr_ir_type(&self, expr: &Expression) -> IRType {
        match &expr.kind {
            ExpressionKind::NumberLiteral(s) => {
                // Se tem ponto, é float, senão int
                if s.contains('.') {
                    IRType::Float
                } else {
                    IRType::Int
                }
            }
            ExpressionKind::StringLiteral(_) => IRType::String,
            ExpressionKind::BoolLiteral(_) => IRType::Bool,
            ExpressionKind::Identifier(name) => {
                if let Some((_, struct_name)) = self.struct_var_map.get(name) {
                    let fields = self
                        .struct_definitions
                        .get(&struct_name)
                        .cloned()
                        .unwrap_or_default();
                    IRType::Struct {
                        name: struct_name,
                        fields,
                    }
                } else if let Some(info) = self.array_map.get(name) {
                    IRType::Array {
                        element_type: Box::new(info.element_type.clone()),
                        size: info.size,
                    }
                } else if let Some(ty) = self.variable_types.get(name) {
                    ty
                } else {
                    IRType::Int
                }
            }
            ExpressionKind::ArrayLiteral { elements } => {
                let elem_type = self.infer_array_element_type(elements);
                IRType::Array {
                    element_type: Box::new(elem_type),
                    size: elements.len(),
                }
            }
            ExpressionKind::TupleLiteral { elements } => {
                let element_types: Vec<IRType> = elements
                    .iter()
                    .map(|e| self.infer_expr_ir_type(e))
                    .collect();
                IRType::Tuple {
                    elements: element_types,
                }
            }
            ExpressionKind::StructLiteral {
                name, type_args, ..
            } => self
                .resolve_struct_type(name, type_args)
                .unwrap_or_else(|| IRType::Struct {
                    name: name.clone(),
                    fields: Vec::new(),
                }),
            ExpressionKind::FieldAccess { object, field } => {
                match self.infer_expr_ir_type(object) {
                    IRType::Struct { fields, .. } => fields
                        .into_iter()
                        .find(|(fname, _)| fname == field)
                        .map(|(_, ty)| ty)
                        .unwrap_or(IRType::Int),
                    _ => IRType::Int,
                }
            }
            ExpressionKind::EnumVariant {
                module_path: _,
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
            } => {
                // Handle StructName::method(args) — the parser treats `Name::Other(...)` as
                // EnumVariant even for struct static/associated-function calls.
                if let Some(fields) = self.struct_definitions.get(enum_name.as_str()) {
                    let mangled = format!("{}_{}", enum_name, variant_name);
                    if let Some(ret) = self.function_return_types.get(&mangled) {
                        return ret.clone();
                    }
                    // Fallback: assume the static call returns an instance of the struct.
                    return IRType::Struct {
                        name: enum_name.clone(),
                        fields: fields.clone(),
                    };
                }
                if self.generic_structs.contains_key(enum_name.as_str()) {
                    let mangled = format!("{}_{}", enum_name, variant_name);
                    if let Some(ret) = self.function_return_types.get(&mangled) {
                        return ret.clone();
                    }
                    return IRType::Struct {
                        name: enum_name.clone(),
                        fields: vec![],
                    };
                }

                let needs_refinement = type_args.is_empty()
                    || type_args
                        .iter()
                        .any(|ann| self.type_annotation_needs_refinement(ann));

                let inferred_args = if needs_refinement {
                    if let Some(data_exprs) = data {
                        self.infer_enum_type_args_from_data(enum_name, variant_name, data_exprs)
                            .or_else(|| self.default_type_args_for_enum(enum_name))
                    } else if let Some(named_fields) = struct_data {
                        self.infer_enum_type_args_from_named_fields(
                            enum_name,
                            variant_name,
                            named_fields,
                        )
                        .or_else(|| self.default_type_args_for_enum(enum_name))
                    } else {
                        self.default_type_args_for_enum(enum_name)
                    }
                } else {
                    None
                };

                let final_args: Vec<TypeAnnotation> = if let Some(args) = inferred_args {
                    args
                } else {
                    type_args.clone()
                };

                self.resolve_enum_type(enum_name, final_args.as_slice())
                    .unwrap_or(IRType::Int)
            }
            ExpressionKind::IndexAccess { array, .. } => match self.infer_expr_ir_type(array) {
                IRType::Array { element_type, .. } => *element_type,
                IRType::String => IRType::Char,
                _ => IRType::Int,
            },
            ExpressionKind::TupleAccess { tuple, index } => match self.infer_expr_ir_type(tuple) {
                IRType::Tuple { elements } if *index < elements.len() => elements[*index].clone(),
                _ => IRType::Int,
            },
            ExpressionKind::Call { callee, arguments } => {
                if let Some(descriptor) = self.host_function_descriptor(callee) {
                    return descriptor.return_type.clone();
                }

                if let ExpressionKind::Identifier(name) = &callee.kind {
                    if let Some(ret) = self.function_return_types.get(name) {
                        return ret.clone();
                    }

                    if self.generic_functions.contains_key(name) {
                        let concrete_types = self.infer_argument_types(arguments);
                        let request = MonomorphizationRequest {
                            generic_name: name.clone(),
                            concrete_types: concrete_types.clone(),
                        };
                        let mangled = request.mangled_name();

                        if let Some(ret) = self.function_return_types.get(&mangled) {
                            return ret.clone();
                        }

                        if let Some(generic_func) = self.generic_functions.get(name) {
                            let mut type_map: HashMap<String, IRType> = HashMap::new();
                            for (param, concrete) in generic_func
                                .type_params
                                .iter()
                                .zip(concrete_types.into_iter())
                            {
                                type_map.insert(param.name.clone(), concrete);
                            }

                            if let Some(ret_ann) = &generic_func.return_type {
                                return self.lower_type_annotation_with_map(ret_ann, &type_map);
                            } else {
                                return IRType::Void;
                            }
                        }
                    }
                }

                IRType::Int
            }
            ExpressionKind::MethodCall {
                object,
                method_name,
                arguments: _,
                type_name,
            } => {
                if let IRType::DynTrait { trait_name } = self.infer_expr_ir_type(object) {
                    if let Some((_, return_type)) = self
                        .trait_method_signatures
                        .get(&trait_name)
                        .and_then(|methods| methods.get(method_name))
                    {
                        return return_type.clone();
                    }
                }

                let obj_type_name = if let Some(name) = type_name {
                    name.clone()
                } else {
                    match self.infer_expr_ir_type(object) {
                        IRType::Struct { name, .. } => name,
                        IRType::Enum { name, .. } => name,
                        _ => return IRType::Int,
                    }
                };

                let function_name = format!("{}_{}", obj_type_name, method_name);

                if let Some(ret) = self.function_return_types.get(&function_name) {
                    ret.clone()
                } else {
                    IRType::Int
                }
            }
            ExpressionKind::If {
                then_block,
                elif_blocks,
                else_block,
                ..
            } => {
                let mut branch_types = Vec::new();

                if let Some(ty) = self.infer_block_result_type(then_block) {
                    branch_types.push(ty);
                }

                for (_, block) in elif_blocks {
                    if let Some(ty) = self.infer_block_result_type(block) {
                        branch_types.push(ty);
                    }
                }

                if let Some(block) = else_block {
                    if let Some(ty) = self.infer_block_result_type(block) {
                        branch_types.push(ty);
                    }
                } else {
                    branch_types.push(IRType::Void);
                }

                self.unify_types(branch_types)
            }
            ExpressionKind::Unless {
                then_block,
                else_block,
                ..
            } => {
                let mut branch_types = Vec::new();

                if let Some(ty) = self.infer_block_result_type(then_block) {
                    branch_types.push(ty);
                }

                if let Some(block) = else_block {
                    if let Some(ty) = self.infer_block_result_type(block) {
                        branch_types.push(ty);
                    }
                } else {
                    branch_types.push(IRType::Void);
                }

                self.unify_types(branch_types)
            }
            ExpressionKind::Match { arms, .. } => {
                let arm_types: Vec<IRType> = arms
                    .iter()
                    .map(|arm| self.infer_expr_ir_type(&arm.body))
                    .collect();
                self.unify_types(arm_types)
            }
            ExpressionKind::Grouping(inner) => self.infer_expr_ir_type(inner),
            ExpressionKind::Unary { operator, operand } => match operator {
                UnaryOperator::Negate => self.infer_expr_ir_type(operand),
                UnaryOperator::Not => IRType::Bool,
            },
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_type = self.infer_expr_ir_type(left);
                let right_type = self.infer_expr_ir_type(right);

                match operator {
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => {
                        // Struct operand: operator is overloaded, return the struct type itself.
                        if let IRType::Struct { .. } = &left_type {
                            return left_type.clone();
                        }
                        let (left_is_float, left_is_string) = match left_type {
                            IRType::Float => (true, false),
                            IRType::String => (false, true),
                            _ => (false, false),
                        };
                        let (right_is_float, right_is_string) = match right_type {
                            IRType::Float => (true, false),
                            IRType::String => (false, true),
                            _ => (false, false),
                        };

                        if left_is_float || right_is_float {
                            IRType::Float
                        } else if left_is_string || right_is_string {
                            IRType::String
                        } else {
                            IRType::Int
                        }
                    }
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or => IRType::Bool,
                }
            }
            ExpressionKind::CharLiteral(_) => IRType::Char,
            ExpressionKind::FString(_) => IRType::String,
            ExpressionKind::Lambda { params, body } => {
                // Return IRType::Function so callers can emit CallIndirect with the right sig.
                let param_types: Vec<IRType> = params
                    .iter()
                    .map(|p| {
                        p.ty.as_ref()
                            .map(|t| self.lower_type_annotation(t))
                            .unwrap_or(IRType::Int)
                    })
                    .collect();
                let ret = self.infer_expr_ir_type(body);
                IRType::Function {
                    params: param_types,
                    return_type: Box::new(ret),
                }
            }
            ExpressionKind::Try(inner) => {
                // `?` unwraps the Ok payload; infer from the inner type's first data field.
                // Until Result<T,E> is a first-class stdlib type, fall back to Int.
                let inner_type = self.infer_expr_ir_type(inner);
                match inner_type {
                    IRType::Enum {
                        name: _,
                        ref variants,
                    } => {
                        // Expect Ok at tag 0 with a single data field
                        if let Some((_, ok_payload)) = variants.first() {
                            if let Some(payload_types) = ok_payload {
                                if let Some(first) = payload_types.first() {
                                    return first.clone();
                                }
                            }
                        }
                        IRType::Int
                    }
                    _ => IRType::Int,
                }
            }
            ExpressionKind::Range { .. } => IRType::Array {
                element_type: Box::new(IRType::Int),
                size: 0,
            },
            ExpressionKind::Block(block) => block
                .statements
                .last()
                .and_then(|stmt| match &stmt.kind {
                    spectra_compiler::ast::StatementKind::Expression(expr) => {
                        Some(self.infer_expr_ir_type(expr))
                    }
                    spectra_compiler::ast::StatementKind::Return(ret) => {
                        ret.value.as_ref().map(|v| self.infer_expr_ir_type(v))
                    }
                    _ => None,
                })
                .unwrap_or(IRType::Void),
            ExpressionKind::DifferentiableBlock(block) => block
                .statements
                .last()
                .and_then(|stmt| match &stmt.kind {
                    spectra_compiler::ast::StatementKind::Expression(expr) => {
                        Some(self.infer_expr_ir_type(expr))
                    }
                    spectra_compiler::ast::StatementKind::Return(ret) => {
                        ret.value.as_ref().map(|v| self.infer_expr_ir_type(v))
                    }
                    _ => None,
                })
                .unwrap_or(IRType::Void),
            ExpressionKind::Cast { target_type, .. } => self.lower_type_annotation(target_type),
        }
    }

    fn infer_pattern_binding_types(
        &self,
        pattern: &spectra_compiler::ast::Pattern,
        scrutinee_enum: Option<&str>,
        scrutinee_type: Option<&IRType>,
        out: &mut HashMap<String, IRType>,
    ) {
        use spectra_compiler::ast::Pattern;

        match pattern {
            Pattern::Wildcard | Pattern::Literal(_) => {}
            Pattern::Identifier(name) => {
                if let Some(ty) = scrutinee_type {
                    out.insert(name.clone(), ty.clone());
                }
            }
            Pattern::Tuple(elements) => {
                if let Some(IRType::Tuple {
                    elements: tuple_types,
                }) = scrutinee_type
                {
                    for (pattern, ty) in elements.iter().zip(tuple_types.iter()) {
                        self.infer_pattern_binding_types(pattern, None, Some(ty), out);
                    }
                }
            }
            Pattern::Struct { fields, .. } => {
                if let Some(IRType::Struct {
                    fields: struct_fields,
                    ..
                }) = scrutinee_type
                {
                    let field_map: HashMap<String, IRType> =
                        struct_fields.iter().cloned().collect();
                    for (field_name, pattern) in fields {
                        if let Some(field_ty) = field_map.get(field_name) {
                            self.infer_pattern_binding_types(pattern, None, Some(field_ty), out);
                        }
                    }
                }
            }
            Pattern::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
                ..
            } => {
                let ordered_patterns: Vec<&spectra_compiler::ast::Pattern> =
                    if let Some(patterns) = data {
                        patterns.iter().collect()
                    } else if let Some(named_patterns) = struct_data {
                        self.reorder_named_variant_patterns(
                            scrutinee_enum.unwrap_or(enum_name),
                            variant_name,
                            named_patterns,
                        )
                        .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                if ordered_patterns.is_empty() {
                    return;
                }

                let mut variants = scrutinee_enum
                    .and_then(|name| self.enum_definitions.get(name).cloned())
                    .or_else(|| {
                        if let Some(IRType::Enum { name, .. }) = scrutinee_type {
                            self.enum_definitions.get(name).cloned()
                        } else {
                            None
                        }
                    })
                    .or_else(|| self.enum_variants_from_ir_type(scrutinee_type))
                    .or_else(|| self.enum_definitions.get(enum_name).cloned());

                if variants.is_none() && !type_args.is_empty() {
                    if let Some(IRType::Enum {
                        variants: specialized,
                        ..
                    }) = self.resolve_enum_type(enum_name, type_args.as_slice())
                    {
                        variants = Some(
                            specialized
                                .into_iter()
                                .enumerate()
                                .map(|(tag, (name, data))| (name, tag, data))
                                .collect(),
                        );
                    }
                }

                if let Some(variants) = variants {
                    if let Some((_, _, variant_types)) =
                        variants.iter().find(|(name, _, _)| name == variant_name)
                    {
                        if let Some(types) = variant_types {
                            for (idx, sub_pattern) in ordered_patterns.iter().enumerate() {
                                if let Some(sub_type) = types.get(idx) {
                                    let next_enum = match sub_type {
                                        IRType::Enum { name, .. } => Some(name.as_str()),
                                        _ => None,
                                    };
                                    self.infer_pattern_binding_types(
                                        sub_pattern,
                                        next_enum,
                                        Some(sub_type),
                                        out,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Pattern::Or(patterns) => {
                if let Some(first) = patterns.first() {
                    self.infer_pattern_binding_types(first, scrutinee_enum, scrutinee_type, out);
                }
            }
        }
    }

    fn infer_match_arm_type(
        &mut self,
        pattern: &spectra_compiler::ast::Pattern,
        body: &Expression,
        scrutinee_enum_name: Option<&str>,
        scrutinee_type: &IRType,
    ) -> IRType {
        let mut bindings = HashMap::new();
        self.infer_pattern_binding_types(
            pattern,
            scrutinee_enum_name,
            Some(scrutinee_type),
            &mut bindings,
        );

        self.variable_types.push_scope();
        for (name, ty) in bindings {
            self.variable_types.insert(name, ty);
        }
        let result = self.infer_expr_ir_type(body);
        self.variable_types.pop_scope();
        result
    }

    fn lower_function(&mut self, ast_func: &ASTFunction) -> IRFunction {
        // Convert parameters
        let params: Vec<Parameter> = ast_func
            .params
            .iter()
            .enumerate()
            .map(|(idx, param)| Parameter {
                id: idx,
                name: param.name.clone(),
                ty: param
                    .ty
                    .as_ref()
                    .map(|t| self.lower_type_annotation(t))
                    .unwrap_or(IRType::Void),
            })
            .collect();

        // Create function
        let return_type = ast_func
            .return_type
            .as_ref()
            .map(|t| self.lower_type_annotation(t))
            .unwrap_or(IRType::Void);

        self.function_return_types
            .insert(ast_func.name.clone(), return_type.clone());

        let mut ir_func = IRFunction::new(&ast_func.name, params.clone(), return_type.clone());

        // Create entry block
        let entry_block = ir_func.add_block("entry");
        self.builder.set_current_block(entry_block);

        // Map parameters to values
        self.value_map.clear();
        self.variable_types.clear();
        self.alloca_map.clear();
        self.array_map.clear();
        self.struct_var_map.clear();
        for (idx, param) in params.iter().enumerate() {
            let value = Value { id: idx };
            self.value_map.insert(param.name.clone(), value);

            if param.ty != IRType::Void {
                self.variable_types
                    .insert(param.name.clone(), param.ty.clone());

                if let IRType::Struct { name, .. } = &param.ty {
                    self.struct_var_map
                        .insert(param.name.clone(), (value, name.clone()));
                }

                if let IRType::Array { element_type, size } = &param.ty {
                    self.array_map.insert(
                        param.name.clone(),
                        ArrayInfo {
                            ptr: value,
                            element_type: element_type.as_ref().clone(),
                            size: *size,
                        },
                    );
                }
            }
        }

        // Analyze which variables are assigned to (need memory allocation)
        let assigned_vars = self.find_assigned_variables(&ast_func.body.statements);

        // Allocate memory for mutable variables
        for var_name in &assigned_vars {
            let alloca_value = self.builder.build_alloca(&mut ir_func, IRType::Int);
            self.alloca_map.insert(var_name.clone(), alloca_value);
        }

        // Lower function body
        self.current_function = Some(ir_func.clone());
        self.current_function_return_annotation = ast_func.return_type.clone();

        // Check if last statement is an expression (implicit return)
        let mut implicit_return_value = None;
        if let Some(last_stmt) = ast_func.body.statements.last() {
            if let StatementKind::Expression(expr) = &last_stmt.kind {
                // Lower all statements except the last
                if ast_func.body.statements.len() > 1 {
                    for stmt in &ast_func.body.statements[..ast_func.body.statements.len() - 1] {
                        self.lower_statement(stmt, &mut ir_func);
                    }
                }
                // Lower the last expression for side effects. Only treat it as an
                // implicit return value when the function does not return void.
                let last_value = self.lower_expression(expr, &mut ir_func);
                if return_type != IRType::Void {
                    implicit_return_value = Some(last_value);
                }
            } else {
                // No implicit return, lower all statements
                self.lower_block(&ast_func.body.statements, &mut ir_func);
            }
        } else {
            // Empty body
            self.lower_block(&ast_func.body.statements, &mut ir_func);
        }

        // Ensure function has a return in the current block
        // (After lowering all statements, we should be in the final block)
        if let Some(current_block_id) = self.builder.get_current_block() {
            if let Some(block) = ir_func.get_block_mut(current_block_id) {
                if block.terminator.is_none() {
                    block.set_terminator(Terminator::Return {
                        value: implicit_return_value,
                    });
                }
            }
        }

        ir_func
    }

    /// Lower an impl block method into a top-level IR function.
    ///
    /// The method `foo` on type `TypeName` becomes a function named `TypeName_foo`
    /// where `self` (in all forms: `self`, `&self`, `&mut self`) is passed as the
    /// first regular parameter of type `TypeName`.
    fn lower_method(&mut self, method: &ASTMethod, type_name: &str) -> IRFunction {
        let mangled_name = format!("{}_{}", type_name, method.name);

        // Convert method parameters to IR parameters.
        // `self` parameters become a parameter typed as the owning struct/enum.
        let params: Vec<Parameter> = method
            .params
            .iter()
            .enumerate()
            .map(|(idx, param)| {
                let ty = if param.is_self {
                    // Resolve the concrete type of `self`
                    if let Some(fields) = self.struct_definitions.get(type_name) {
                        IRType::Struct {
                            name: type_name.to_string(),
                            fields: fields.clone(),
                        }
                    } else if let Some(variants) = self.enum_definitions.get(type_name) {
                        let simplified = variants
                            .iter()
                            .map(|(vn, _, data)| (vn.clone(), data.clone()))
                            .collect();
                        IRType::Enum {
                            name: type_name.to_string(),
                            variants: simplified,
                        }
                    } else {
                        // Type not yet registered — use a named struct as placeholder.
                        IRType::Struct {
                            name: type_name.to_string(),
                            fields: vec![],
                        }
                    }
                } else {
                    param
                        .type_annotation
                        .as_ref()
                        .map(|t| self.lower_type_annotation_with_self(t, type_name))
                        .unwrap_or(IRType::Void)
                };
                Parameter {
                    id: idx,
                    name: param.name.clone(),
                    ty,
                }
            })
            .collect();

        let return_type = method
            .return_type
            .as_ref()
            .map(|t| self.lower_type_annotation_with_self(t, type_name))
            .unwrap_or(IRType::Void);

        self.function_return_types
            .insert(mangled_name.clone(), return_type.clone());

        let mut ir_func = IRFunction::new(&mangled_name, params.clone(), return_type.clone());
        let entry_block = ir_func.add_block("entry");
        self.builder.set_current_block(entry_block);

        // Reset per-function lowering state (mirrors lower_function).
        self.value_map.clear();
        self.variable_types.clear();
        self.alloca_map.clear();
        self.array_map.clear();
        self.struct_var_map.clear();

        for (idx, param) in params.iter().enumerate() {
            let value = Value { id: idx };
            self.value_map.insert(param.name.clone(), value);

            if param.ty != IRType::Void {
                self.variable_types
                    .insert(param.name.clone(), param.ty.clone());

                if let IRType::Struct { name, .. } = &param.ty {
                    self.struct_var_map
                        .insert(param.name.clone(), (value, name.clone()));
                }

                if let IRType::Array { element_type, size } = &param.ty {
                    self.array_map.insert(
                        param.name.clone(),
                        ArrayInfo {
                            ptr: value,
                            element_type: element_type.as_ref().clone(),
                            size: *size,
                        },
                    );
                }
            }
        }

        // Allocate slots for variables that are assigned inside the body.
        let assigned_vars = self.find_assigned_variables(&method.body.statements);
        for var_name in &assigned_vars {
            let alloca_value = self.builder.build_alloca(&mut ir_func, IRType::Int);
            self.alloca_map.insert(var_name.clone(), alloca_value);
        }

        self.current_function = Some(ir_func.clone());
        self.current_function_return_annotation = method.return_type.clone();

        // Lower the body; support implicit returns (last expression = return value).
        let mut implicit_return_value = None;
        if let Some(last_stmt) = method.body.statements.last() {
            if let StatementKind::Expression(expr) = &last_stmt.kind {
                if method.body.statements.len() > 1 {
                    for stmt in &method.body.statements[..method.body.statements.len() - 1] {
                        self.lower_statement(stmt, &mut ir_func);
                    }
                }
                let last_value = self.lower_expression(expr, &mut ir_func);
                if return_type != IRType::Void {
                    implicit_return_value = Some(last_value);
                }
            } else {
                self.lower_block(&method.body.statements, &mut ir_func);
            }
        } else {
            self.lower_block(&method.body.statements, &mut ir_func);
        }

        // Seal the current block with a return terminator if one is missing.
        if let Some(current_block_id) = self.builder.get_current_block() {
            if let Some(block) = ir_func.get_block_mut(current_block_id) {
                if block.terminator.is_none() {
                    block.set_terminator(Terminator::Return {
                        value: implicit_return_value,
                    });
                }
            }
        }

        ir_func
    }

    fn lower_type_annotation_with_self(
        &self,
        annotation: &TypeAnnotation,
        self_type_name: &str,
    ) -> IRType {
        match &annotation.kind {
            TypeAnnotationKind::Simple { segments }
                if segments.len() == 1 && segments[0] == "Self" =>
            {
                if let Some(fields) = self.struct_definitions.get(self_type_name) {
                    IRType::Struct {
                        name: self_type_name.to_string(),
                        fields: fields.clone(),
                    }
                } else if let Some(variants) = self.enum_definitions.get(self_type_name) {
                    let simplified = variants
                        .iter()
                        .map(|(variant_name, _, data)| (variant_name.clone(), data.clone()))
                        .collect();
                    IRType::Enum {
                        name: self_type_name.to_string(),
                        variants: simplified,
                    }
                } else {
                    IRType::Struct {
                        name: self_type_name.to_string(),
                        fields: vec![],
                    }
                }
            }
            _ => self.lower_type_annotation(annotation),
        }
    }

    /// Lower a lambda expression into a self-contained top-level IR function.
    ///
    /// Saves and restores all per-function state so nested lambdas work correctly.
    /// Returns the generated IR function; the caller must queue it in `pending_lambdas`.
    fn lower_lambda(
        &mut self,
        name: String,
        captures: &[ClosureCapture],
        params: &[spectra_compiler::ast::LambdaParam],
        body: &Expression,
    ) -> IRFunction {
        use crate::ir::Parameter;

        // Slot 0 is the hidden closure environment handle. User-visible
        // parameters start at slot 1 and keep the public `fn(...) -> ...` type.
        let mut ir_params: Vec<Parameter> = vec![Parameter {
            id: 0,
            name: "__closure_env".to_string(),
            ty: IRType::Int,
        }];
        ir_params.extend(params.iter().enumerate().map(|(idx, p)| {
            Parameter {
                id: idx + 1,
                name: p.name.clone(),
                ty: p
                    .ty
                    .as_ref()
                    .map(|t| self.lower_type_annotation(t))
                    .unwrap_or(IRType::Int),
            }
        }));

        // Infer the return type from the body expression
        let return_type = self.infer_expr_ir_type(body);

        // Register the function so recursive/forward references resolve correctly
        self.function_return_types
            .insert(name.clone(), return_type.clone());

        // --- Save outer function state ---
        let saved_value_map = self.value_map.clone();
        let saved_variable_types = self.variable_types.clone();
        let saved_alloca_map = std::mem::take(&mut self.alloca_map);
        let saved_array_map = self.array_map.clone();
        let saved_struct_var_map = self.struct_var_map.clone();
        let saved_current_function = self.current_function.take();
        let saved_builder_block = self.builder.get_current_block();

        // --- Reset state for lambda body ---
        self.value_map.clear();
        self.variable_types.clear();
        self.alloca_map = HashMap::new();
        self.array_map.clear();
        self.struct_var_map.clear();

        let mut lambda_func = IRFunction::new(&name, ir_params.clone(), return_type.clone());
        let entry_block = lambda_func.add_block("entry");
        self.builder.set_current_block(entry_block);

        let env_value = Value { id: 0 };
        self.value_map
            .insert("__closure_env".to_string(), env_value);
        self.variable_types
            .insert("__closure_env".to_string(), IRType::Int);

        for (slot, capture) in captures.iter().enumerate() {
            let index = self
                .builder
                .build_const_int(&mut lambda_func, (slot + 1) as i64);
            let ptr =
                self.builder
                    .build_getelementptr(&mut lambda_func, env_value, index, IRType::Int);
            let value = self
                .builder
                .build_load_typed(&mut lambda_func, ptr, capture.ty.clone());
            self.value_map.insert(capture.name.clone(), value);
            self.variable_types
                .insert(capture.name.clone(), capture.ty.clone());
        }

        // Map explicit parameters into value/type maps
        for param in ir_params.iter().skip(1) {
            let value = Value { id: param.id };
            self.value_map.insert(param.name.clone(), value);
            if param.ty != IRType::Void {
                self.variable_types
                    .insert(param.name.clone(), param.ty.clone());
            }
        }

        // Pre-allocate mutable slots for any variables assigned inside the body
        let assigned_vars = if let ExpressionKind::Block(block) = &body.kind {
            self.find_assigned_variables(&block.statements)
        } else {
            std::collections::HashSet::new()
        };
        for var_name in &assigned_vars {
            let alloca_value = self.builder.build_alloca(&mut lambda_func, IRType::Int);
            self.alloca_map.insert(var_name.clone(), alloca_value);
        }

        // Lower the body
        let result_value = self.lower_expression(body, &mut lambda_func);

        // Emit return if the current block has no terminator yet
        if let Some(cur_block_id) = self.builder.get_current_block() {
            if let Some(block) = lambda_func.get_block_mut(cur_block_id) {
                if block.terminator.is_none() {
                    if return_type != IRType::Void {
                        block.set_terminator(Terminator::Return {
                            value: Some(result_value),
                        });
                    } else {
                        block.set_terminator(Terminator::Return { value: None });
                    }
                }
            }
        }

        // --- Restore outer function state ---
        self.value_map = saved_value_map;
        self.variable_types = saved_variable_types;
        self.alloca_map = saved_alloca_map;
        self.array_map = saved_array_map;
        self.struct_var_map = saved_struct_var_map;
        self.current_function = saved_current_function;
        if let Some(block_id) = saved_builder_block {
            self.builder.set_current_block(block_id);
        }

        lambda_func
    }

    fn build_closure_object(
        &mut self,
        ir_func: &mut IRFunction,
        lambda_name: String,
        captures: &[ClosureCapture],
    ) -> Value {
        let slots = captures.len() + 1;
        let closure_ty = IRType::Array {
            element_type: Box::new(IRType::Int),
            size: slots,
        };
        let closure_handle = self.builder.build_alloca(ir_func, closure_ty);

        let code_ptr = self.builder.build_func_addr(ir_func, lambda_name);
        let zero = self.builder.build_const_int(ir_func, 0);
        let code_slot =
            self.builder
                .build_getelementptr(ir_func, closure_handle, zero, IRType::Int);
        self.builder.build_store(ir_func, code_slot, code_ptr);

        for (idx, capture) in captures.iter().enumerate() {
            let capture_value = self.lower_identifier_value(&capture.name, ir_func);
            let slot_index = self.builder.build_const_int(ir_func, (idx + 1) as i64);
            let slot =
                self.builder
                    .build_getelementptr(ir_func, closure_handle, slot_index, IRType::Int);
            self.builder.build_store(ir_func, slot, capture_value);
        }

        closure_handle
    }

    fn lower_closure_handle_call(
        &mut self,
        closure_handle: Value,
        mut arg_values: Vec<Value>,
        public_params: Vec<IRType>,
        public_return: IRType,
        ir_func: &mut IRFunction,
    ) -> Value {
        let zero = self.builder.build_const_int(ir_func, 0);
        let code_slot =
            self.builder
                .build_getelementptr(ir_func, closure_handle, zero, IRType::Int);
        let code_ptr = self.builder.build_load(ir_func, code_slot);

        let mut call_args = Vec::with_capacity(arg_values.len() + 1);
        call_args.push(closure_handle);
        call_args.append(&mut arg_values);

        let mut signature_params = Vec::with_capacity(public_params.len() + 1);
        signature_params.push(IRType::Int);
        signature_params.extend(public_params);

        self.builder
            .build_call_indirect(
                ir_func,
                code_ptr,
                call_args,
                signature_params,
                public_return.clone(),
            )
            .unwrap_or_else(|| {
                if public_return == IRType::Void {
                    self.builder.build_const_int(ir_func, 0)
                } else {
                    ir_func.next_value()
                }
            })
    }

    fn lower_identifier_value(&mut self, name: &str, ir_func: &mut IRFunction) -> Value {
        if let Some(value) = self.const_values.get(name).cloned() {
            self.emit_const_value(&value, ir_func)
        } else if let Some(info) = self.array_map.get(name) {
            info.ptr
        } else if let Some((struct_ptr, _)) = self.struct_var_map.get(name) {
            struct_ptr
        } else if let Some(&alloca_ptr) = self.alloca_map.get(name) {
            self.builder.build_load(ir_func, alloca_ptr)
        } else if let Some(value) = self.value_map.get(name) {
            value
        } else {
            ir_func.next_value()
        }
    }

    fn collect_lambda_captures(
        &self,
        params: &[spectra_compiler::ast::LambdaParam],
        body: &Expression,
    ) -> Vec<ClosureCapture> {
        let mut locals: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut captures = Vec::new();
        let mut seen = HashSet::new();
        self.collect_lambda_captures_expr(body, &mut locals, &mut captures, &mut seen);
        captures
    }

    fn collect_lambda_captures_expr(
        &self,
        expr: &Expression,
        locals: &mut HashSet<String>,
        captures: &mut Vec<ClosureCapture>,
        seen: &mut HashSet<String>,
    ) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                if !locals.contains(name) && seen.insert(name.clone()) {
                    if let Some(ty) = self.variable_types.get(name) {
                        captures.push(ClosureCapture {
                            name: name.clone(),
                            ty,
                        });
                    }
                }
            }
            ExpressionKind::Binary { left, right, .. } => {
                self.collect_lambda_captures_expr(left, locals, captures, seen);
                self.collect_lambda_captures_expr(right, locals, captures, seen);
            }
            ExpressionKind::Unary { operand, .. } | ExpressionKind::Try(operand) => {
                self.collect_lambda_captures_expr(operand, locals, captures, seen);
            }
            ExpressionKind::Call { callee, arguments } => {
                self.collect_lambda_captures_expr(callee, locals, captures, seen);
                for arg in arguments {
                    self.collect_lambda_captures_expr(arg, locals, captures, seen);
                }
            }
            ExpressionKind::MethodCall {
                object, arguments, ..
            } => {
                self.collect_lambda_captures_expr(object, locals, captures, seen);
                for arg in arguments {
                    self.collect_lambda_captures_expr(arg, locals, captures, seen);
                }
            }
            ExpressionKind::Lambda {
                params: nested_params,
                body,
            } => {
                let mut nested_locals = locals.clone();
                for param in nested_params {
                    nested_locals.insert(param.name.clone());
                }
                self.collect_lambda_captures_expr(body, &mut nested_locals, captures, seen);
            }
            ExpressionKind::Block(block) => {
                let mut block_locals = locals.clone();
                for stmt in &block.statements {
                    self.collect_lambda_captures_stmt(stmt, &mut block_locals, captures, seen);
                }
            }
            ExpressionKind::DifferentiableBlock(block) => {
                let mut block_locals = locals.clone();
                for stmt in &block.statements {
                    self.collect_lambda_captures_stmt(stmt, &mut block_locals, captures, seen);
                }
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.collect_lambda_captures_expr(condition, locals, captures, seen);
                self.collect_lambda_captures_block(then_block, locals, captures, seen);
                for (elif_condition, elif_block) in elif_blocks {
                    self.collect_lambda_captures_expr(elif_condition, locals, captures, seen);
                    self.collect_lambda_captures_block(elif_block, locals, captures, seen);
                }
                if let Some(block) = else_block {
                    self.collect_lambda_captures_block(block, locals, captures, seen);
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                self.collect_lambda_captures_expr(condition, locals, captures, seen);
                self.collect_lambda_captures_block(then_block, locals, captures, seen);
                if let Some(block) = else_block {
                    self.collect_lambda_captures_block(block, locals, captures, seen);
                }
            }
            ExpressionKind::Grouping(inner) => {
                self.collect_lambda_captures_expr(inner, locals, captures, seen);
            }
            ExpressionKind::FieldAccess { object, .. } => {
                self.collect_lambda_captures_expr(object, locals, captures, seen);
            }
            ExpressionKind::TupleAccess { tuple, .. } => {
                self.collect_lambda_captures_expr(tuple, locals, captures, seen);
            }
            ExpressionKind::IndexAccess { array, index } => {
                self.collect_lambda_captures_expr(array, locals, captures, seen);
                self.collect_lambda_captures_expr(index, locals, captures, seen);
            }
            ExpressionKind::ArrayLiteral { elements }
            | ExpressionKind::TupleLiteral { elements } => {
                for element in elements {
                    self.collect_lambda_captures_expr(element, locals, captures, seen);
                }
            }
            ExpressionKind::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.collect_lambda_captures_expr(value, locals, captures, seen);
                }
            }
            ExpressionKind::EnumVariant {
                data, struct_data, ..
            } => {
                if let Some(values) = data {
                    for value in values {
                        self.collect_lambda_captures_expr(value, locals, captures, seen);
                    }
                }
                if let Some(fields) = struct_data {
                    for (_, value) in fields {
                        self.collect_lambda_captures_expr(value, locals, captures, seen);
                    }
                }
            }
            ExpressionKind::Match { scrutinee, arms } => {
                self.collect_lambda_captures_expr(scrutinee, locals, captures, seen);
                for arm in arms {
                    let mut arm_locals = locals.clone();
                    Self::collect_pattern_names(&arm.pattern, &mut arm_locals);
                    if let Some(guard) = &arm.guard {
                        self.collect_lambda_captures_expr(guard, &mut arm_locals, captures, seen);
                    }
                    self.collect_lambda_captures_expr(&arm.body, &mut arm_locals, captures, seen);
                }
            }
            ExpressionKind::Cast { expr, .. } => {
                self.collect_lambda_captures_expr(expr, locals, captures, seen);
            }
            ExpressionKind::FString(parts) => {
                for part in parts {
                    if let FStringPart::Interpolated(expr) = part {
                        self.collect_lambda_captures_expr(expr, locals, captures, seen);
                    }
                }
            }
            ExpressionKind::Range { start, end, .. } => {
                self.collect_lambda_captures_expr(start, locals, captures, seen);
                self.collect_lambda_captures_expr(end, locals, captures, seen);
            }
            ExpressionKind::NumberLiteral(_)
            | ExpressionKind::StringLiteral(_)
            | ExpressionKind::BoolLiteral(_)
            | ExpressionKind::CharLiteral(_) => {}
        }
    }

    fn collect_lambda_captures_block(
        &self,
        block: &Block,
        locals: &HashSet<String>,
        captures: &mut Vec<ClosureCapture>,
        seen: &mut HashSet<String>,
    ) {
        let mut block_locals = locals.clone();
        for stmt in &block.statements {
            self.collect_lambda_captures_stmt(stmt, &mut block_locals, captures, seen);
        }
    }

    fn collect_lambda_captures_stmt(
        &self,
        stmt: &Statement,
        locals: &mut HashSet<String>,
        captures: &mut Vec<ClosureCapture>,
        seen: &mut HashSet<String>,
    ) {
        match &stmt.kind {
            StatementKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    self.collect_lambda_captures_expr(value, locals, captures, seen);
                }
                Self::collect_pattern_names(&let_stmt.pattern, locals);
            }
            StatementKind::Assignment(assign) => {
                self.collect_lvalue_captures(&assign.target, locals, captures, seen);
                self.collect_lambda_captures_expr(&assign.value, locals, captures, seen);
            }
            StatementKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_lambda_captures_expr(value, locals, captures, seen);
                }
            }
            StatementKind::Expression(expr) => {
                self.collect_lambda_captures_expr(expr, locals, captures, seen);
            }
            StatementKind::While(loop_stmt) => {
                self.collect_lambda_captures_expr(&loop_stmt.condition, locals, captures, seen);
                self.collect_lambda_captures_block(&loop_stmt.body, locals, captures, seen);
            }
            StatementKind::DoWhile(loop_stmt) => {
                self.collect_lambda_captures_block(&loop_stmt.body, locals, captures, seen);
                self.collect_lambda_captures_expr(&loop_stmt.condition, locals, captures, seen);
            }
            StatementKind::For(for_loop) => {
                self.collect_lambda_captures_expr(&for_loop.iterable, locals, captures, seen);
                let mut loop_locals = locals.clone();
                loop_locals.insert(for_loop.iterator.clone());
                self.collect_lambda_captures_block(&for_loop.body, &loop_locals, captures, seen);
            }
            StatementKind::IfLet(stmt) => {
                self.collect_lambda_captures_expr(&stmt.value, locals, captures, seen);
                let mut then_locals = locals.clone();
                Self::collect_pattern_names(&stmt.pattern, &mut then_locals);
                self.collect_lambda_captures_block(&stmt.then_block, &then_locals, captures, seen);
                if let Some(block) = &stmt.else_block {
                    self.collect_lambda_captures_block(block, locals, captures, seen);
                }
            }
            StatementKind::WhileLet(stmt) => {
                self.collect_lambda_captures_expr(&stmt.value, locals, captures, seen);
                let mut body_locals = locals.clone();
                Self::collect_pattern_names(&stmt.pattern, &mut body_locals);
                self.collect_lambda_captures_block(&stmt.body, &body_locals, captures, seen);
            }
            StatementKind::Loop(loop_stmt) => {
                self.collect_lambda_captures_block(&loop_stmt.body, locals, captures, seen);
            }
            StatementKind::Switch(switch_stmt) => {
                self.collect_lambda_captures_expr(&switch_stmt.value, locals, captures, seen);
                for case in &switch_stmt.cases {
                    self.collect_lambda_captures_expr(&case.pattern, locals, captures, seen);
                    self.collect_lambda_captures_block(&case.body, locals, captures, seen);
                }
                if let Some(block) = &switch_stmt.default {
                    self.collect_lambda_captures_block(block, locals, captures, seen);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }

    fn collect_lvalue_captures(
        &self,
        target: &spectra_compiler::ast::LValue,
        locals: &mut HashSet<String>,
        captures: &mut Vec<ClosureCapture>,
        seen: &mut HashSet<String>,
    ) {
        match target {
            spectra_compiler::ast::LValue::Identifier(name) => {
                if !locals.contains(name) && seen.insert(name.clone()) {
                    if let Some(ty) = self.variable_types.get(name) {
                        captures.push(ClosureCapture {
                            name: name.clone(),
                            ty,
                        });
                    }
                }
            }
            spectra_compiler::ast::LValue::IndexAccess { array, index } => {
                self.collect_lambda_captures_expr(array, locals, captures, seen);
                self.collect_lambda_captures_expr(index, locals, captures, seen);
            }
            spectra_compiler::ast::LValue::FieldAccess { object, .. } => {
                self.collect_lambda_captures_expr(object, locals, captures, seen);
            }
        }
    }

    fn collect_pattern_names(
        pattern: &spectra_compiler::ast::Pattern,
        names: &mut HashSet<String>,
    ) {
        match pattern {
            spectra_compiler::ast::Pattern::Identifier(name) => {
                names.insert(name.clone());
            }
            spectra_compiler::ast::Pattern::Tuple(items) => {
                for item in items {
                    Self::collect_pattern_names(item, names);
                }
            }
            spectra_compiler::ast::Pattern::Struct { fields, .. } => {
                for (_, pattern) in fields {
                    Self::collect_pattern_names(pattern, names);
                }
            }
            spectra_compiler::ast::Pattern::EnumVariant {
                data, struct_data, ..
            } => {
                if let Some(items) = data {
                    for item in items {
                        Self::collect_pattern_names(item, names);
                    }
                }
                if let Some(fields) = struct_data {
                    for (_, item) in fields {
                        Self::collect_pattern_names(item, names);
                    }
                }
            }
            spectra_compiler::ast::Pattern::Or(patterns) => {
                for pattern in patterns {
                    Self::collect_pattern_names(pattern, names);
                }
            }
            spectra_compiler::ast::Pattern::Wildcard
            | spectra_compiler::ast::Pattern::Literal(_) => {}
        }
    }

    fn collect_default_trait_methods(
        &self,
        trait_name: &str,
        explicit_methods: &[ASTMethod],
    ) -> Vec<ASTMethod> {
        let mut seen: std::collections::HashSet<String> =
            explicit_methods.iter().map(|m| m.name.clone()).collect();
        let mut out = Vec::new();
        self.collect_default_trait_methods_recursive(trait_name, &mut seen, &mut out);
        out
    }

    fn collect_default_trait_methods_recursive(
        &self,
        trait_name: &str,
        seen: &mut std::collections::HashSet<String>,
        out: &mut Vec<ASTMethod>,
    ) {
        let Some(trait_decl) = self.trait_declarations.get(trait_name) else {
            return;
        };

        for parent_trait in &trait_decl.parent_traits {
            self.collect_default_trait_methods_recursive(parent_trait, seen, out);
        }

        for method in &trait_decl.methods {
            if method.body.is_none() || seen.contains(&method.name) {
                continue;
            }

            seen.insert(method.name.clone());
            out.push(ASTMethod {
                name: method.name.clone(),
                params: method.params.clone(),
                return_type: method.return_type.clone(),
                body: method.body.clone().unwrap_or(Block {
                    span: method.span,
                    statements: Vec::new(),
                }),
                span: method.span,
                visibility: Visibility::Public,
            });
        }
    }

    fn lower_block(&mut self, statements: &[Statement], ir_func: &mut IRFunction) {
        self.lower_block_with_scope(statements, ir_func, true);
    }

    fn lower_block_with_scope(
        &mut self,
        statements: &[Statement],
        ir_func: &mut IRFunction,
        create_scope: bool,
    ) {
        if create_scope {
            self.value_map.push_scope();
            self.variable_types.push_scope();
            self.array_map.push_scope();
            self.struct_var_map.push_scope();
        }

        for stmt in statements {
            self.lower_statement(stmt, ir_func);
        }

        if create_scope {
            // Drop semantics: before leaving the scope, call `StructName_drop(ptr)` for every
            // struct variable whose type implements the `Drop` trait (in reverse order).
            let drop_calls: Vec<(String, Value)> = self
                .struct_var_map
                .scopes
                .last()
                .map(|scope| {
                    scope
                        .values()
                        .filter(|(_, struct_name)| {
                            self.trait_implementations
                                .contains_key(&(struct_name.clone(), "Drop".to_string()))
                        })
                        .map(|(ptr, struct_name)| (struct_name.clone(), *ptr))
                        .collect()
                })
                .unwrap_or_default();
            for (struct_name, ptr) in drop_calls.iter().rev() {
                let fn_name = format!("{}_drop", struct_name);
                self.builder.build_call(ir_func, fn_name, vec![*ptr], false);
            }

            self.struct_var_map.pop_scope();
            self.array_map.pop_scope();
            self.variable_types.pop_scope();
            self.value_map.pop_scope();
        }
    }
    fn find_assigned_variables(
        &self,
        statements: &[Statement],
    ) -> std::collections::HashSet<String> {
        use std::collections::HashSet;
        let mut assigned = HashSet::new();

        for stmt in statements {
            match &stmt.kind {
                StatementKind::Assignment(assign) => {
                    // Extract variable name from LValue
                    // For now, only track simple identifiers (not array elements)
                    if let spectra_compiler::ast::LValue::Identifier(name) = &assign.target {
                        assigned.insert(name.clone());
                    }
                }
                StatementKind::While(while_stmt) => {
                    // Recursively check loop body
                    assigned.extend(self.find_assigned_variables(&while_stmt.body.statements));
                }
                StatementKind::DoWhile(do_while) => {
                    assigned.extend(self.find_assigned_variables(&do_while.body.statements));
                }
                StatementKind::For(for_stmt) => {
                    assigned.extend(self.find_assigned_variables(&for_stmt.body.statements));
                }
                StatementKind::Loop(loop_stmt) => {
                    assigned.extend(self.find_assigned_variables(&loop_stmt.body.statements));
                }
                StatementKind::WhileLet(while_let) => {
                    assigned.extend(self.find_assigned_variables(&while_let.body.statements));
                }
                StatementKind::IfLet(if_let) => {
                    assigned.extend(self.find_assigned_variables(&if_let.then_block.statements));
                    if let Some(else_b) = &if_let.else_block {
                        assigned.extend(self.find_assigned_variables(&else_b.statements));
                    }
                }
                StatementKind::Switch(switch) => {
                    for case in &switch.cases {
                        assigned.extend(self.find_assigned_variables(&case.body.statements));
                    }
                    if let Some(default) = &switch.default {
                        assigned.extend(self.find_assigned_variables(&default.statements));
                    }
                }
                StatementKind::Expression(expr) => {
                    // Check if expression contains assignments in blocks
                    if let ExpressionKind::If {
                        then_block,
                        elif_blocks,
                        else_block,
                        ..
                    } = &expr.kind
                    {
                        assigned.extend(self.find_assigned_variables(&then_block.statements));
                        for (_, block) in elif_blocks {
                            assigned.extend(self.find_assigned_variables(&block.statements));
                        }
                        if let Some(else_b) = else_block {
                            assigned.extend(self.find_assigned_variables(&else_b.statements));
                        }
                    }
                }
                _ => {}
            }
        }

        assigned
    }

    fn lower_branch_block_result(
        &mut self,
        block: &Block,
        ir_func: &mut IRFunction,
        entry_block: usize,
    ) -> (Option<Value>, usize, bool) {
        self.value_map.push_scope();
        self.variable_types.push_scope();
        self.array_map.push_scope();
        self.struct_var_map.push_scope();

        // Lower all statements and extract the value of the last expression.
        // Previously this called lower_block (which lowers ALL stmts) and then
        // re-evaluated the last stmt with lower_expression, causing the last
        // statement to execute TWICE. Fix: lower all-but-last with lower_block,
        // then handle the last stmt specially to capture its value without duplication.
        let stmts = &block.statements;
        let produced_value = if stmts.is_empty() {
            None
        } else {
            for stmt in &stmts[..stmts.len() - 1] {
                self.lower_statement(stmt, ir_func);
            }
            let last = &stmts[stmts.len() - 1];
            match &last.kind {
                StatementKind::Expression(expr) => Some(self.lower_expression(expr, ir_func)),
                _ => {
                    self.lower_statement(last, ir_func);
                    None
                }
            }
        };

        let current_block_id = self.builder.get_current_block().unwrap_or(entry_block);

        let has_terminator = ir_func
            .get_block(current_block_id)
            .map(|block| block.terminator.is_some())
            .unwrap_or(false);

        self.struct_var_map.pop_scope();
        self.array_map.pop_scope();
        self.variable_types.pop_scope();
        self.value_map.pop_scope();

        (produced_value, current_block_id, has_terminator)
    }

    fn evaluate_int_constant(&self, expr: &Expression) -> Option<i64> {
        match &expr.kind {
            ExpressionKind::NumberLiteral(value) => value.parse::<i64>().ok(),
            ExpressionKind::BoolLiteral(value) => Some(if *value { 1 } else { 0 }),
            ExpressionKind::Grouping(inner) => self.evaluate_int_constant(inner),
            ExpressionKind::Unary { operator, operand } => {
                let inner = self.evaluate_int_constant(operand)?;
                match operator {
                    UnaryOperator::Negate => inner.checked_neg(),
                    UnaryOperator::Not => Some(if inner == 0 { 1 } else { 0 }),
                }
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let lhs = self.evaluate_int_constant(left)?;
                let rhs = self.evaluate_int_constant(right)?;
                match operator {
                    BinaryOperator::Add => lhs.checked_add(rhs),
                    BinaryOperator::Subtract => lhs.checked_sub(rhs),
                    BinaryOperator::Multiply => lhs.checked_mul(rhs),
                    BinaryOperator::Divide => {
                        if rhs == 0 {
                            None
                        } else {
                            Some(lhs / rhs)
                        }
                    }
                    BinaryOperator::Modulo => {
                        if rhs == 0 {
                            None
                        } else {
                            Some(lhs % rhs)
                        }
                    }
                    BinaryOperator::Equal => Some(if lhs == rhs { 1 } else { 0 }),
                    BinaryOperator::NotEqual => Some(if lhs != rhs { 1 } else { 0 }),
                    BinaryOperator::Less => Some(if lhs < rhs { 1 } else { 0 }),
                    BinaryOperator::Greater => Some(if lhs > rhs { 1 } else { 0 }),
                    BinaryOperator::LessEqual => Some(if lhs <= rhs { 1 } else { 0 }),
                    BinaryOperator::GreaterEqual => Some(if lhs >= rhs { 1 } else { 0 }),
                    BinaryOperator::And => Some(if lhs != 0 && rhs != 0 { 1 } else { 0 }),
                    BinaryOperator::Or => Some(if lhs != 0 || rhs != 0 { 1 } else { 0 }),
                }
            }
            _ => None,
        }
    }

    fn lower_tensor_literal(
        &mut self,
        expr: &Expression,
        dtype: &IRType,
        rank: Option<usize>,
        ir_func: &mut IRFunction,
    ) -> Option<Value> {
        let ExpressionKind::ArrayLiteral { elements } = &expr.kind else {
            return None;
        };

        match rank {
            Some(1) => {
                let mut args = Vec::with_capacity(elements.len() + 1);
                args.push(self.builder.build_const_int(ir_func, elements.len() as i64));
                for element in elements {
                    args.push(self.lower_expression(element, ir_func));
                }
                let host = match dtype {
                    IRType::Float => "spectra.std.tensor.literal_f",
                    IRType::Int => "spectra.std.tensor.literal",
                    _ => return None,
                };
                self.builder
                    .build_host_call(ir_func, host.to_string(), args, true)
            }
            Some(2) => {
                let mut rows = Vec::with_capacity(elements.len());
                let mut cols: Option<usize> = None;
                for row in elements {
                    let ExpressionKind::ArrayLiteral {
                        elements: row_elements,
                    } = &row.kind
                    else {
                        return None;
                    };
                    if let Some(expected_cols) = cols {
                        if row_elements.len() != expected_cols {
                            return None;
                        }
                    } else {
                        cols = Some(row_elements.len());
                    }
                    rows.push(row_elements);
                }

                let cols = cols.unwrap_or(0);
                let flat_len = elements.len().saturating_mul(cols);
                let mut args = Vec::with_capacity(flat_len + 2);
                args.push(self.builder.build_const_int(ir_func, elements.len() as i64));
                args.push(self.builder.build_const_int(ir_func, cols as i64));
                for row in rows {
                    for element in row {
                        args.push(self.lower_expression(element, ir_func));
                    }
                }
                let host = match dtype {
                    IRType::Float => "spectra.std.tensor.literal2_f",
                    IRType::Int => "spectra.std.tensor.literal2",
                    _ => return None,
                };
                self.builder
                    .build_host_call(ir_func, host.to_string(), args, true)
            }
            _ => None,
        }
    }

    fn lower_statement(&mut self, stmt: &Statement, ir_func: &mut IRFunction) {
        match &stmt.kind {
            StatementKind::Let(let_stmt) => {
                let binding_name = match &let_stmt.pattern {
                    spectra_compiler::ast::Pattern::Identifier(name) => Some(name.clone()),
                    _ => None,
                };

                // Discover variable type either from initializer or annotation
                let inferred_type = if let Some(ref value_expr) = let_stmt.value {
                    Some(self.infer_expr_ir_type(value_expr))
                } else if let Some(ref type_ann) = let_stmt.ty {
                    Some(self.lower_type_annotation(type_ann))
                } else {
                    None
                };

                if let (Some(name), Some(ty)) = (binding_name.as_ref(), inferred_type.as_ref()) {
                    self.variable_types.insert(name.clone(), (*ty).clone());
                }

                if let Some(ref value_expr) = let_stmt.value {
                    // Track the lambda function name BEFORE lowering so closure_var_map
                    // is populated even if the lambda itself modifies lambda_counter.
                    let is_lambda_binding =
                        matches!(&value_expr.kind, ExpressionKind::Lambda { .. });

                    let annotated_tensor_type = let_stmt.ty.as_ref().and_then(|type_ann| {
                        let ty = self.lower_type_annotation(type_ann);
                        matches!(ty, IRType::Tensor { .. }).then_some(ty)
                    });
                    let value = if let Some(IRType::Tensor { dtype, rank, .. }) =
                        annotated_tensor_type.as_ref()
                    {
                        if let Some(value) =
                            self.lower_tensor_literal(value_expr, dtype, *rank, ir_func)
                        {
                            value
                        } else {
                            self.lower_expression(value_expr, ir_func)
                        }
                    } else {
                        self.lower_expression(value_expr, ir_func)
                    };

                    // Register in closure_var_map when the value bound is a lambda
                    if let Some(name) = binding_name.as_ref().filter(|_| is_lambda_binding) {
                        if let Some(IRType::Function {
                            params,
                            return_type,
                        }) = inferred_type.clone()
                        {
                            self.closure_var_map.insert(
                                name.clone(),
                                ClosureInfo {
                                    signature_params: params,
                                    signature_return: *return_type,
                                },
                            );
                        }
                    }

                    if binding_name.is_none() {
                        let scrutinee_type = inferred_type.as_ref();
                        let scrutinee_enum = match scrutinee_type {
                            Some(IRType::Enum { name, .. }) => Some(name.as_str()),
                            _ => None,
                        };
                        self.lower_pattern_bindings(
                            &let_stmt.pattern,
                            value,
                            scrutinee_enum,
                            scrutinee_type,
                            ir_func,
                        );
                        return;
                    }

                    let name = binding_name.expect("identifier pattern should be present");

                    match &value_expr.kind {
                        ExpressionKind::ArrayLiteral { .. } => {
                            if let Some(IRType::Array { element_type, size }) =
                                inferred_type.clone()
                            {
                                self.array_map.insert(
                                    name.clone(),
                                    ArrayInfo {
                                        ptr: value,
                                        element_type: *element_type,
                                        size,
                                    },
                                );
                            }
                            self.value_map.insert(name.clone(), value);
                        }
                        ExpressionKind::StructLiteral {
                            name: struct_name,
                            type_args,
                            ..
                        } => {
                            let (actual_name, _) =
                                self.ensure_struct_definition(struct_name, type_args.as_slice());
                            self.struct_var_map
                                .insert(name.clone(), (value, actual_name.clone()));
                            self.value_map.insert(name.clone(), value);
                        }
                        _ => {
                            if let Some(ref type_ann) = let_stmt.ty {
                                let var_type = self.lower_type_annotation(type_ann);
                                if let IRType::Struct {
                                    name: struct_type_name,
                                    fields,
                                } = var_type
                                {
                                    let struct_ptr = self.builder.build_alloca(
                                        ir_func,
                                        IRType::Struct {
                                            name: struct_type_name.clone(),
                                            fields: fields.clone(),
                                        },
                                    );
                                    self.builder.build_store(ir_func, struct_ptr, value);
                                    self.struct_var_map
                                        .insert(name.clone(), (struct_ptr, struct_type_name));
                                    self.value_map.insert(name.clone(), struct_ptr);
                                } else if let Some(&alloca_ptr) = self.alloca_map.get(&name) {
                                    self.builder.build_store(ir_func, alloca_ptr, value);
                                } else {
                                    self.value_map.insert(name.clone(), value);
                                }
                            } else if let Some(&alloca_ptr) = self.alloca_map.get(&name) {
                                self.builder.build_store(ir_func, alloca_ptr, value);
                            } else {
                                self.value_map.insert(name.clone(), value);
                            }
                        }
                    }
                }
            }
            StatementKind::Assignment(assign) => {
                let value = self.lower_expression(&assign.value, ir_func);

                match &assign.target {
                    spectra_compiler::ast::LValue::Identifier(name) => {
                        // Assignment to simple variable (uses memory)
                        if let Some(&alloca_ptr) = self.alloca_map.get(name) {
                            self.builder.build_store(ir_func, alloca_ptr, value);

                            if let Some((_, struct_name)) = self.struct_var_map.get(name) {
                                self.struct_var_map
                                    .insert(name.clone(), (alloca_ptr, struct_name));
                            }

                            if let Some(IRType::Array { element_type, size }) =
                                self.variable_types.get(name)
                            {
                                self.array_map.insert(
                                    name.clone(),
                                    ArrayInfo {
                                        ptr: alloca_ptr,
                                        element_type: *element_type,
                                        size,
                                    },
                                );
                            }
                        } else {
                            // Fallback: update value_map (shouldn't happen if analysis is correct)
                            self.value_map.insert(name.clone(), value);

                            if let Some(var_ty) = self.variable_types.get(name) {
                                match var_ty {
                                    IRType::Struct {
                                        name: struct_name, ..
                                    } => {
                                        self.struct_var_map
                                            .insert(name.clone(), (value, struct_name.clone()));
                                    }
                                    IRType::Array { element_type, size } => {
                                        self.array_map.insert(
                                            name.clone(),
                                            ArrayInfo {
                                                ptr: value,
                                                element_type: *element_type,
                                                size,
                                            },
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    spectra_compiler::ast::LValue::IndexAccess { array, index } => {
                        // Assignment to array element
                        let array_ptr = self.lower_expression(array, ir_func);
                        let index_value = self.lower_expression(index, ir_func);

                        // Calcular endereço do elemento
                        let elem_type = match self.infer_expr_ir_type(array) {
                            IRType::Array { element_type, .. } => *element_type,
                            IRType::String => IRType::Int,
                            _ => IRType::Int,
                        };
                        let elem_ptr = self.builder.build_getelementptr(
                            ir_func,
                            array_ptr,
                            index_value,
                            elem_type,
                        );

                        // Store valor no elemento
                        self.builder.build_store(ir_func, elem_ptr, value);
                    }
                    spectra_compiler::ast::LValue::FieldAccess { object, field } => {
                        // Assignment to struct field (e.g. self.x = ...)
                        // Step 1: collect the field info before any mutable borrow
                        let field_info: Option<(usize, IRType)> =
                            if let spectra_compiler::ast::ExpressionKind::Identifier(var_name) =
                                &object.kind
                            {
                                let lookup = self.struct_var_map.get(var_name.as_str());
                                if let Some((_, sname)) = lookup {
                                    let sname = sname.clone();
                                    self.struct_definitions.get(&sname).and_then(|defs| {
                                        defs.iter()
                                            .enumerate()
                                            .find(|(_, (fname, _))| {
                                                fname.as_str() == field.as_str()
                                            })
                                            .map(|(idx, (_, ty))| (idx, ty.clone()))
                                    })
                                } else {
                                    None
                                }
                            } else {
                                match self.infer_expr_ir_type(object) {
                                    IRType::Struct { fields, .. } => fields
                                        .into_iter()
                                        .enumerate()
                                        .find(|(_, (fname, _))| fname.as_str() == field.as_str())
                                        .map(|(idx, (_, ty))| (idx, ty)),
                                    _ => None,
                                }
                            };

                        // Step 2: get (or compute) the struct pointer
                        let struct_ptr =
                            if let spectra_compiler::ast::ExpressionKind::Identifier(var_name) =
                                &object.kind
                            {
                                if let Some((ptr, _)) = self.struct_var_map.get(var_name.as_str()) {
                                    ptr
                                } else {
                                    self.lower_expression(object, ir_func)
                                }
                            } else {
                                self.lower_expression(object, ir_func)
                            };

                        // Step 3: GEP + store
                        if let Some((field_idx, field_type)) = field_info {
                            let index_value =
                                self.builder.build_const_int(ir_func, field_idx as i64);
                            let field_ptr = self.builder.build_getelementptr(
                                ir_func,
                                struct_ptr,
                                index_value,
                                field_type,
                            );
                            self.builder.build_store(ir_func, field_ptr, value);
                        }
                    }
                }
            }
            StatementKind::Return(ret) => {
                let value = ret
                    .value
                    .as_ref()
                    .map(|expr| self.lower_expression(expr, ir_func));
                self.builder.build_return(ir_func, value);
            }
            StatementKind::Expression(expr) => {
                self.lower_expression(expr, ir_func);
            }
            StatementKind::While(while_stmt) => {
                let header_block = ir_func.add_block("while.header");
                let body_block = ir_func.add_block("while.body");
                let exit_block = ir_func.add_block("while.exit");

                // Branch to header
                self.builder.build_branch(ir_func, header_block);
                self.builder.set_current_block(header_block);

                // Evaluate condition
                let condition = self.lower_expression(&while_stmt.condition, ir_func);
                self.builder
                    .build_cond_branch(ir_func, condition, body_block, exit_block);

                // Body (push loop context for break/continue)
                self.loop_stack.push(LoopContext {
                    header_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);
                self.lower_block(&while_stmt.body.statements, ir_func);
                self.builder.build_branch(ir_func, header_block);
                self.loop_stack.pop();

                // Exit
                self.builder.set_current_block(exit_block);
            }
            StatementKind::DoWhile(do_while) => {
                let body_block = ir_func.add_block("do_while.body");
                let header_block = ir_func.add_block("do_while.header");
                let exit_block = ir_func.add_block("do_while.exit");

                // Branch to body first
                self.builder.build_branch(ir_func, body_block);

                // Body (push loop context for break/continue)
                self.loop_stack.push(LoopContext {
                    header_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);
                self.lower_block(&do_while.body.statements, ir_func);
                self.builder.build_branch(ir_func, header_block);
                self.loop_stack.pop();

                // Header/condition
                self.builder.set_current_block(header_block);
                let condition = self.lower_expression(&do_while.condition, ir_func);
                self.builder
                    .build_cond_branch(ir_func, condition, body_block, exit_block);

                // Exit
                self.builder.set_current_block(exit_block);
            }
            StatementKind::For(for_stmt) => {
                // Check if iterable is a range expression — handle as an integer range loop
                // rather than loading array elements.
                if let ExpressionKind::Range {
                    start,
                    end,
                    inclusive,
                } = &for_stmt.iterable.kind
                {
                    let start_val = self.lower_expression(start, ir_func);
                    let end_val = self.lower_expression(end, ir_func);
                    let is_inclusive = *inclusive;

                    let header_block = ir_func.add_block("range.header");
                    let body_block = ir_func.add_block("range.body");
                    let increment_block = ir_func.add_block("range.increment");
                    let exit_block = ir_func.add_block("range.exit");

                    // Allocate and initialise loop index to start
                    let index_alloca = self.builder.build_alloca(ir_func, IRType::Int);
                    self.builder.build_store(ir_func, index_alloca, start_val);

                    // Load end_val into an alloca so it remains stable across iterations
                    let end_alloca = self.builder.build_alloca(ir_func, IRType::Int);
                    self.builder.build_store(ir_func, end_alloca, end_val);

                    self.builder.build_branch(ir_func, header_block);

                    // Header: check index < end (exclusive) or index <= end (inclusive)
                    self.builder.set_current_block(header_block);
                    let current_index = self.builder.build_load(ir_func, index_alloca);
                    let current_end = self.builder.build_load(ir_func, end_alloca);
                    let condition = if is_inclusive {
                        self.builder.build_le(ir_func, current_index, current_end)
                    } else {
                        self.builder.build_lt(ir_func, current_index, current_end)
                    };
                    self.builder
                        .build_cond_branch(ir_func, condition, body_block, exit_block);

                    // Body block
                    self.builder.set_current_block(body_block);
                    self.loop_stack.push(LoopContext {
                        header_block: increment_block,
                        exit_block,
                    });

                    self.value_map.push_scope();
                    self.variable_types.push_scope();
                    self.array_map.push_scope();
                    self.struct_var_map.push_scope();

                    let body_index = self.builder.build_load(ir_func, index_alloca);
                    self.value_map.insert(for_stmt.iterator.clone(), body_index);
                    self.variable_types
                        .insert(for_stmt.iterator.clone(), IRType::Int);

                    self.lower_block_with_scope(&for_stmt.body.statements, ir_func, false);

                    if let Some(current_block) = self.builder.get_current_block() {
                        if let Some(block) = ir_func.get_block_mut(current_block) {
                            if block.terminator.is_none() {
                                self.builder.build_branch(ir_func, increment_block);
                            }
                        }
                    }

                    self.struct_var_map.pop_scope();
                    self.array_map.pop_scope();
                    self.variable_types.pop_scope();
                    self.value_map.pop_scope();
                    self.loop_stack.pop();

                    // Increment block: index += 1
                    self.builder.set_current_block(increment_block);
                    let step_index = self.builder.build_load(ir_func, index_alloca);
                    let one = self.builder.build_const_int(ir_func, 1);
                    let next_index = self.builder.build_add(ir_func, step_index, one);
                    self.builder.build_store(ir_func, index_alloca, next_index);
                    self.builder.build_branch(ir_func, header_block);

                    self.builder.set_current_block(exit_block);
                } else {
                    // Lower iterable expression once to avoid recomputation
                    let iterable_value = self.lower_expression(&for_stmt.iterable, ir_func);
                    let iterable_type = self.infer_expr_ir_type(&for_stmt.iterable);

                    let (element_type, length) = match iterable_type {
                        IRType::Array { element_type, size } => (*element_type, size),
                        other => {
                            self.error(format!(
                                "for-loop lowering currently supports arrays only, found {:?}",
                                other
                            ));
                            (IRType::Int, 0)
                        }
                    };

                    let header_block = ir_func.add_block("for.header");
                    let body_block = ir_func.add_block("for.body");
                    let increment_block = ir_func.add_block("for.increment");
                    let exit_block = ir_func.add_block("for.exit");

                    // Allocate and initialise loop index
                    let index_alloca = self.builder.build_alloca(ir_func, IRType::Int);
                    let zero = self.builder.build_const_int(ir_func, 0);
                    self.builder.build_store(ir_func, index_alloca, zero);

                    // Jump to header to evaluate guard
                    self.builder.build_branch(ir_func, header_block);

                    // Header: check index < length
                    self.builder.set_current_block(header_block);
                    let current_index = self.builder.build_load(ir_func, index_alloca);
                    let length_const = self.builder.build_const_int(ir_func, length as i64);
                    let condition = self.builder.build_lt(ir_func, current_index, length_const);
                    self.builder
                        .build_cond_branch(ir_func, condition, body_block, exit_block);

                    // Body block
                    self.builder.set_current_block(body_block);
                    self.loop_stack.push(LoopContext {
                        header_block: increment_block,
                        exit_block,
                    });

                    // Scoped bindings for iterator variable
                    self.value_map.push_scope();
                    self.variable_types.push_scope();
                    self.array_map.push_scope();
                    self.struct_var_map.push_scope();

                    let body_index = self.builder.build_load(ir_func, index_alloca);
                    let element_ptr = self.builder.build_getelementptr(
                        ir_func,
                        iterable_value,
                        body_index,
                        element_type.clone(),
                    );
                    let element_value =
                        self.builder
                            .build_load_typed(ir_func, element_ptr, element_type.clone());

                    // Bind iterator variable in current scope
                    self.value_map
                        .insert(for_stmt.iterator.clone(), element_value);
                    self.variable_types
                        .insert(for_stmt.iterator.clone(), element_type.clone());

                    if let IRType::Struct { name, .. } = &element_type {
                        self.struct_var_map
                            .insert(for_stmt.iterator.clone(), (element_value, name.clone()));
                    }

                    self.lower_block_with_scope(&for_stmt.body.statements, ir_func, false);

                    // Determine if body naturally falls through
                    if let Some(current_block) = self.builder.get_current_block() {
                        if let Some(block) = ir_func.get_block_mut(current_block) {
                            if block.terminator.is_none() {
                                self.builder.build_branch(ir_func, increment_block);
                            }
                        }
                    }

                    self.struct_var_map.pop_scope();
                    self.array_map.pop_scope();
                    self.variable_types.pop_scope();
                    self.value_map.pop_scope();
                    self.loop_stack.pop();

                    // Increment block
                    self.builder.set_current_block(increment_block);
                    let step_index = self.builder.build_load(ir_func, index_alloca);
                    let one = self.builder.build_const_int(ir_func, 1);
                    let next_index = self.builder.build_add(ir_func, step_index, one);
                    self.builder.build_store(ir_func, index_alloca, next_index);
                    self.builder.build_branch(ir_func, header_block);

                    // Exit block becomes current for following statements
                    self.builder.set_current_block(exit_block);
                } // end else (array-based for)
            }
            StatementKind::Loop(loop_stmt) => {
                let body_block = ir_func.add_block("loop.body");
                let exit_block = ir_func.add_block("loop.exit");

                // Branch to body
                self.builder.build_branch(ir_func, body_block);

                // Body (infinite loop - needs break to exit)
                // Use body_block as header since it's the loop entry point
                self.loop_stack.push(LoopContext {
                    header_block: body_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);
                self.lower_block(&loop_stmt.body.statements, ir_func);
                self.builder.build_branch(ir_func, body_block);
                self.loop_stack.pop();

                // Exit (unreachable unless break is used)
                self.builder.set_current_block(exit_block);
            }
            StatementKind::Switch(switch) => {
                let scrutinee = self.lower_expression(&switch.value, ir_func);

                // Create blocks for each case and default/exit
                let exit_block = ir_func.add_block("switch.exit");
                let mut cases = Vec::new();
                let mut case_blocks = Vec::new();

                for (idx, case) in switch.cases.iter().enumerate() {
                    let case_block = ir_func.add_block(&format!("switch.case.{}", idx));
                    case_blocks.push((case_block, case));

                    // Extract constant value from pattern
                    let pattern_int = self
                        .evaluate_int_constant(&case.pattern)
                        .unwrap_or_else(|| {
                            self.error(format!(
                                "Switch case pattern must be a constant integer expression, found {:?}",
                                case.pattern.kind
                            ));
                            0
                        });
                    cases.push((pattern_int, case_block));
                }

                // Build switch terminator
                let default = if switch.default.is_some() {
                    ir_func.add_block("switch.default")
                } else {
                    exit_block
                };

                if let Some(current_block) = self.builder.get_current_block() {
                    if let Some(block) = ir_func.get_block_mut(current_block) {
                        block.set_terminator(Terminator::Switch {
                            value: scrutinee,
                            cases,
                            default,
                        });
                    }
                }

                // Lower each case body
                for (case_block, case) in case_blocks {
                    self.builder.set_current_block(case_block);
                    self.lower_block(&case.body.statements, ir_func);
                    self.builder.build_branch(ir_func, exit_block);
                }

                // Lower default if present
                if let Some(ref default_block) = switch.default {
                    self.builder.set_current_block(default);
                    self.lower_block(&default_block.statements, ir_func);
                    self.builder.build_branch(ir_func, exit_block);
                }

                // Exit
                self.builder.set_current_block(exit_block);
            }
            StatementKind::Break => {
                // Branch to the exit block of the innermost loop
                if let Some(loop_ctx) = self.loop_stack.last() {
                    self.builder.build_branch(ir_func, loop_ctx.exit_block);
                } else {
                    // Break outside of loop - error, but generate unreachable
                    if let Some(current_block) = self.builder.get_current_block() {
                        if let Some(block) = ir_func.get_block_mut(current_block) {
                            block.set_terminator(Terminator::Unreachable);
                        }
                    }
                }
            }
            StatementKind::Continue => {
                // Branch to the header block of the innermost loop
                if let Some(loop_ctx) = self.loop_stack.last() {
                    self.builder.build_branch(ir_func, loop_ctx.header_block);
                } else {
                    // Continue outside of loop - error, but generate unreachable
                    if let Some(current_block) = self.builder.get_current_block() {
                        if let Some(block) = ir_func.get_block_mut(current_block) {
                            block.set_terminator(Terminator::Unreachable);
                        }
                    }
                }
            }
            StatementKind::IfLet(IfLetStatement {
                pattern,
                value,
                then_block,
                else_block,
                ..
            }) => {
                // Evaluate the scrutinee expression once
                let scrutinee_value = self.lower_expression(value, ir_func);
                let scrutinee_type = self.infer_expr_ir_type(value);
                let scrutinee_enum_name = if let IRType::Enum { name, .. } = &scrutinee_type {
                    Some(name.clone())
                } else {
                    None
                };

                // Create basic blocks
                let then_blk = ir_func.add_block("if_let.then");
                let exit_blk = ir_func.add_block("if_let.exit");
                let else_blk_opt = else_block
                    .as_ref()
                    .map(|_| ir_func.add_block("if_let.else"));
                let false_target = else_blk_opt.unwrap_or(exit_blk);

                // Pattern check in the current block
                let matches = self.lower_pattern_check(
                    pattern,
                    scrutinee_value,
                    scrutinee_enum_name.as_deref(),
                    Some(&scrutinee_type),
                    ir_func,
                );
                self.builder
                    .build_cond_branch(ir_func, matches, then_blk, false_target);

                // --- Then block ---
                self.builder.set_current_block(then_blk);
                // Push an inner scope for the pattern bindings
                self.value_map.push_scope();
                self.variable_types.push_scope();
                self.array_map.push_scope();
                self.struct_var_map.push_scope();

                self.lower_pattern_bindings(
                    pattern,
                    scrutinee_value,
                    scrutinee_enum_name.as_deref(),
                    Some(&scrutinee_type),
                    ir_func,
                );
                // lower_block creates its own inner scope; bindings remain visible via scope search
                self.lower_block(&then_block.statements, ir_func);

                self.struct_var_map.pop_scope();
                self.array_map.pop_scope();
                self.variable_types.pop_scope();
                self.value_map.pop_scope();

                let cur = self.builder.get_current_block().unwrap_or(then_blk);
                let terminated = ir_func
                    .get_block(cur)
                    .map(|b| b.terminator.is_some())
                    .unwrap_or(false);
                if !terminated {
                    self.builder.build_branch(ir_func, exit_blk);
                }

                // --- Else block (optional) ---
                if let (Some(else_b), Some(else_blk)) = (else_block, else_blk_opt) {
                    self.builder.set_current_block(else_blk);
                    self.lower_block(&else_b.statements, ir_func);
                    let cur = self.builder.get_current_block().unwrap_or(else_blk);
                    let terminated = ir_func
                        .get_block(cur)
                        .map(|b| b.terminator.is_some())
                        .unwrap_or(false);
                    if !terminated {
                        self.builder.build_branch(ir_func, exit_blk);
                    }
                }

                // --- Exit block ---
                self.builder.set_current_block(exit_blk);
            }
            StatementKind::WhileLet(WhileLetStatement {
                pattern,
                value,
                body,
                ..
            }) => {
                let header_block = ir_func.add_block("while_let.header");
                let body_block = ir_func.add_block("while_let.body");
                let exit_block = ir_func.add_block("while_let.exit");

                // Jump from current block into loop header
                self.builder.build_branch(ir_func, header_block);
                self.builder.set_current_block(header_block);

                // Re-evaluate scrutinee on every iteration and check the pattern
                let scrutinee_value = self.lower_expression(value, ir_func);
                let scrutinee_type = self.infer_expr_ir_type(value);
                let scrutinee_enum_name = if let IRType::Enum { name, .. } = &scrutinee_type {
                    Some(name.clone())
                } else {
                    None
                };

                let matches = self.lower_pattern_check(
                    pattern,
                    scrutinee_value,
                    scrutinee_enum_name.as_deref(),
                    Some(&scrutinee_type),
                    ir_func,
                );
                self.builder
                    .build_cond_branch(ir_func, matches, body_block, exit_block);

                // --- Body block ---
                // Register loop so that break/continue work correctly
                self.loop_stack.push(LoopContext {
                    header_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);

                // Push scope for pattern bindings (visible to all statements in the body)
                self.value_map.push_scope();
                self.variable_types.push_scope();
                self.array_map.push_scope();
                self.struct_var_map.push_scope();

                // Bind pattern variables (e.g. `n` in `while let Option::Some(n) = ...`)
                // header_block dominates body_block, so scrutinee_value is live here.
                self.lower_pattern_bindings(
                    pattern,
                    scrutinee_value,
                    scrutinee_enum_name.as_deref(),
                    Some(&scrutinee_type),
                    ir_func,
                );
                self.lower_block(&body.statements, ir_func);

                self.struct_var_map.pop_scope();
                self.array_map.pop_scope();
                self.variable_types.pop_scope();
                self.value_map.pop_scope();

                self.loop_stack.pop();

                // Back-edge to header unless body already has a terminator
                let cur = self.builder.get_current_block().unwrap_or(body_block);
                let terminated = ir_func
                    .get_block(cur)
                    .map(|b| b.terminator.is_some())
                    .unwrap_or(false);
                if !terminated {
                    self.builder.build_branch(ir_func, header_block);
                }

                // --- Exit block ---
                self.builder.set_current_block(exit_block);
            }
        }
    }

    /// Infer concrete types from argument expressions
    /// This is a simplified type inference for monomorphization
    fn infer_argument_types(&self, arguments: &[Expression]) -> Vec<IRType> {
        arguments
            .iter()
            .map(|arg| {
                // Try to infer type from expression
                match &arg.kind {
                    ExpressionKind::NumberLiteral(n) => {
                        // Try to determine if int or float
                        if n.contains('.') {
                            IRType::Float
                        } else {
                            IRType::Int
                        }
                    }
                    ExpressionKind::BoolLiteral(_) => IRType::Bool,
                    ExpressionKind::StringLiteral(_) => IRType::Pointer(Box::new(IRType::Int)), // String is pointer
                    ExpressionKind::Identifier(name) => {
                        // Try to find in struct_var_map
                        if let Some((_, struct_name)) = self.struct_var_map.get(name) {
                            // Get fields from struct_definitions
                            let fields = self
                                .struct_definitions
                                .get(&struct_name)
                                .cloned()
                                .unwrap_or_default();
                            IRType::Struct {
                                name: struct_name,
                                fields,
                            }
                        } else if let Some(info) = self.array_map.get(name) {
                            IRType::Array {
                                element_type: Box::new(info.element_type.clone()),
                                size: info.size,
                            }
                        } else if let Some(ty) = self.variable_types.get(name) {
                            ty
                        } else {
                            // Default to Int if we can't determine
                            IRType::Int
                        }
                    }
                    ExpressionKind::StructLiteral {
                        name, type_args, ..
                    } => self
                        .resolve_struct_type(name, type_args)
                        .unwrap_or(IRType::Struct {
                            name: name.clone(),
                            fields: Vec::new(),
                        }),
                    _ => IRType::Int, // Default fallback
                }
            })
            .collect()
    }

    fn resolve_call_path(&self, callee: &Expression) -> Option<Vec<String>> {
        match &callee.kind {
            ExpressionKind::Identifier(name) => Some(vec![name.clone()]),
            ExpressionKind::FieldAccess { object, field } => {
                let mut path = self.resolve_call_path(object)?;
                path.push(field.clone());
                Some(path)
            }
            _ => None,
        }
    }

    fn host_function_descriptor(&self, callee: &Expression) -> Option<HostFunctionDescriptor> {
        let path = self.resolve_call_path(callee)?;
        self.host_function_descriptor_for_path(&path)
    }

    fn host_function_descriptor_for_path(&self, path: &[String]) -> Option<HostFunctionDescriptor> {
        // Direct path lookup (e.g. std.io.print).
        if let Some(desc) = lookup_std_host_function(&path) {
            return Some(desc);
        }
        // Fallback: resolve single-segment bare names via std_import_aliases
        // (e.g. `print` after `import std.io`).
        if path.len() == 1 {
            if let Some(full_path) = self.std_import_aliases.get(&path[0]) {
                return lookup_std_host_function(full_path);
            }
        }
        // Fallback: resolve two-segment alias.function paths via std_import_aliases
        // (e.g. `str.len` after `import std.string as str;`).
        if path.len() == 2 {
            let alias_key = &path[0];
            let func_name = &path[1];
            if let Some(full_prefix) = self.std_import_aliases.get(alias_key) {
                // full_prefix is e.g. ["spectra","std","string","len"] for a bare import,
                // but for alias lookup we need the module prefix (all but last segment)
                // stored per-alias. Try building ["std", module, func].
                // We use the stdlib_path stored in the module exports via aliases:
                // find any alias key matching alias.func and resolve.
                let composed_key = format!("{}.{}", alias_key, func_name);
                if let Some(full_path2) = self.std_import_aliases.get(&composed_key) {
                    return lookup_std_host_function(full_path2);
                }
                // Also try: the full_prefix ends with the bare func name, so the
                // module prefix is full_prefix[..len-1].join and we append func_name.
                if full_prefix.len() >= 2 {
                    let module_prefix = &full_prefix[..full_prefix.len() - 1];
                    let mut resolved = module_prefix.to_vec();
                    resolved.push(func_name.clone());
                    if let Some(desc) = lookup_std_host_function(&resolved) {
                        return Some(desc);
                    }
                }
            }
        }
        None
    }

    fn lower_expression(&mut self, expr: &Expression, ir_func: &mut IRFunction) -> Value {
        match &expr.kind {
            ExpressionKind::NumberLiteral(n) => {
                // Try to parse as integer first, then float
                if let Ok(int_val) = n.parse::<i64>() {
                    self.builder.build_const_int(ir_func, int_val)
                } else if let Ok(float_val) = n.parse::<f64>() {
                    self.builder.build_const_float(ir_func, float_val)
                } else {
                    // Fallback to 0 if parsing fails
                    self.builder.build_const_int(ir_func, 0)
                }
            }
            ExpressionKind::StringLiteral(s) => self.lower_string_literal(s, ir_func),
            ExpressionKind::BoolLiteral(b) => self.builder.build_const_bool(ir_func, *b),
            ExpressionKind::Identifier(name) => {
                // Check if this is an array - return pointer directly
                if let Some(value) = self.const_values.get(name).cloned() {
                    self.emit_const_value(&value, ir_func)
                }
                // Check if this is an array - return pointer directly
                else if let Some(info) = self.array_map.get(name) {
                    info.ptr
                }
                // Check if this is a struct variable
                else if let Some((struct_ptr, _)) = self.struct_var_map.get(name) {
                    // Struct variables are represented as pointers — return the pointer directly.
                    // Field access via FieldAccess/struct_var_map uses the pointer for GEP;
                    // method calls receive the pointer as `self`.
                    struct_ptr
                }
                // Check if variable is in memory (mutable)
                else if let Some(&alloca_ptr) = self.alloca_map.get(name) {
                    // Load from memory
                    self.builder.build_load(ir_func, alloca_ptr)
                } else if let Some(value) = self.value_map.get(name) {
                    // Use SSA value directly
                    value
                } else {
                    // Unknown variable, create placeholder
                    ir_func.next_value()
                }
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_ir_type = self.infer_expr_ir_type(left);

                // Operator overloading: if the left operand is a struct, dispatch to the
                // correspondingly named method (e.g. `Point_add(lhs, rhs)`).
                if let IRType::Struct { name: ref sn, .. } = left_ir_type {
                    if let Some(method_name) = operator_overload_method(operator) {
                        let lhs = self.lower_expression(left, ir_func);
                        let rhs = self.lower_expression(right, ir_func);
                        let fn_name = format!("{}_{}", sn, method_name);
                        return self
                            .builder
                            .build_call(ir_func, fn_name, vec![lhs, rhs], true)
                            .unwrap_or_else(|| ir_func.next_value());
                    }
                }

                let lhs = self.lower_expression(left, ir_func);
                let rhs = self.lower_expression(right, ir_func);

                match operator {
                    BinaryOperator::Add => self.builder.build_add(ir_func, lhs, rhs),
                    BinaryOperator::Subtract => self.builder.build_sub(ir_func, lhs, rhs),
                    BinaryOperator::Multiply => self.builder.build_mul(ir_func, lhs, rhs),
                    BinaryOperator::Divide => self.builder.build_div(ir_func, lhs, rhs),
                    BinaryOperator::Modulo => self.builder.build_rem(ir_func, lhs, rhs),
                    BinaryOperator::Equal => self.builder.build_eq(ir_func, lhs, rhs),
                    BinaryOperator::NotEqual => self.builder.build_ne(ir_func, lhs, rhs),
                    BinaryOperator::Less => self.builder.build_lt(ir_func, lhs, rhs),
                    BinaryOperator::LessEqual => self.builder.build_le(ir_func, lhs, rhs),
                    BinaryOperator::Greater => self.builder.build_gt(ir_func, lhs, rhs),
                    BinaryOperator::GreaterEqual => self.builder.build_ge(ir_func, lhs, rhs),
                    BinaryOperator::And => self.builder.build_and(ir_func, lhs, rhs),
                    BinaryOperator::Or => self.builder.build_or(ir_func, lhs, rhs),
                }
            }
            ExpressionKind::Unary { operator, operand } => {
                use spectra_compiler::ast::UnaryOperator;

                // Operator overloading: if operand is a struct, dispatch `StructName_neg`.
                if matches!(operator, UnaryOperator::Negate) {
                    let op_ir_type = self.infer_expr_ir_type(operand);
                    if let IRType::Struct { name: ref sn, .. } = op_ir_type {
                        let val = self.lower_expression(operand, ir_func);
                        let fn_name = format!("{}_neg", sn);
                        return self
                            .builder
                            .build_call(ir_func, fn_name, vec![val], true)
                            .unwrap_or_else(|| ir_func.next_value());
                    }
                }

                let operand_value = self.lower_expression(operand, ir_func);

                match operator {
                    UnaryOperator::Negate => {
                        // Negate: 0 - operand, preserving numeric kind.
                        let zero = match self.infer_expr_ir_type(operand) {
                            IRType::Float => self.builder.build_const_float(ir_func, 0.0),
                            _ => self.builder.build_const_int(ir_func, 0),
                        };
                        self.builder.build_sub(ir_func, zero, operand_value)
                    }
                    UnaryOperator::Not => self.builder.build_not(ir_func, operand_value),
                }
            }
            ExpressionKind::Call { callee, arguments } => {
                let arg_values: Vec<Value> = arguments
                    .iter()
                    .map(|arg| self.lower_expression(arg, ir_func))
                    .collect();

                if let Some(descriptor) = self.host_function_descriptor(callee) {
                    // Special case: io.print / io.println / io.eprint / io.eprintln
                    // use (type_tag, value) pairs so the runtime can dispatch the
                    // correct formatter per argument.
                    if descriptor.runtime_name == "spectra.std.io.print"
                        || descriptor.runtime_name == "spectra.std.io.println"
                        || descriptor.runtime_name == "spectra.std.io.eprint"
                        || descriptor.runtime_name == "spectra.std.io.eprintln"
                    {
                        let mut paired: Vec<Value> = Vec::with_capacity(arg_values.len() * 2);
                        for (arg_val, arg_expr) in arg_values.iter().zip(arguments.iter()) {
                            let tag: i64 = match self.infer_expr_ir_type(arg_expr) {
                                IRType::String => 1, // PRINT_TAG_STR
                                IRType::Bool => 2,   // PRINT_TAG_BOOL
                                IRType::Float => 3,  // PRINT_TAG_FLOAT
                                _ => 0,              // PRINT_TAG_INT
                            };
                            let tag_val = self.builder.build_const_int(ir_func, tag);
                            paired.push(tag_val);
                            paired.push(*arg_val);
                        }
                        let result_value = self.builder.build_host_call(
                            ir_func,
                            descriptor.runtime_name.to_string(),
                            paired,
                            descriptor.returns_value,
                        );
                        return result_value.unwrap_or_else(|| ir_func.next_value());
                    }

                    let result_value = self.builder.build_host_call(
                        ir_func,
                        descriptor.runtime_name.to_string(),
                        arg_values.clone(),
                        descriptor.returns_value,
                    );
                    return result_value.unwrap_or_else(|| ir_func.next_value());
                }

                // Extract function name from callee
                let function_name = if let ExpressionKind::Identifier(name) = &callee.kind {
                    name.clone()
                } else {
                    "unknown".to_string()
                };

                // --- Closure variable call: direct name lookup ---
                // Function values are closure handles: slot 0 stores the code pointer
                // and the handle itself is passed as hidden environment argument.
                if let ExpressionKind::Identifier(name) = &callee.kind {
                    if let Some(info) = self.closure_var_map.get(name).cloned() {
                        if let Some(handle) = self.value_map.get(name) {
                            return self.lower_closure_handle_call(
                                handle,
                                arg_values,
                                info.signature_params,
                                info.signature_return,
                                ir_func,
                            );
                        }
                    }

                    // --- Function pointer parameter (fn(T) -> R) ---
                    // If the identifier is a variable of Function type, call through the pointer.
                    if let Some(var_type) = self.variable_types.get(name) {
                        if let IRType::Function {
                            params: sig_params,
                            return_type: sig_return,
                        } = var_type.clone()
                        {
                            if let Some(fn_ptr) = self.value_map.get(name) {
                                return self.lower_closure_handle_call(
                                    fn_ptr,
                                    arg_values,
                                    sig_params,
                                    *sig_return,
                                    ir_func,
                                );
                            }
                        }
                    }
                }

                // temporary bypass for closures
                if !self.function_return_types.contains_key(&function_name)
                    && !self.generic_functions.contains_key(&function_name)
                    && function_name != "unknown"
                {
                    return self.builder.build_const_int(ir_func, 0);
                }

                // Check if this is a call to a generic function
                let final_function_name = if self.generic_functions.contains_key(&function_name) {
                    // This is a generic function call - we need to infer concrete types
                    // For now, we'll infer types from the argument expressions
                    let concrete_types = self.infer_argument_types(arguments);

                    let request = MonomorphizationRequest {
                        generic_name: function_name.clone(),
                        concrete_types: concrete_types.clone(),
                    };

                    let mangled = request.mangled_name();

                    // Check if we already generated this specialization
                    if !self.generated_specializations.contains_key(&mangled) {
                        // Mark it as pending
                        // requesting specialization
                        self.pending_specializations.push(request);
                    }

                    mangled
                } else {
                    function_name
                };

                self.builder
                    .build_call(ir_func, final_function_name, arg_values, true)
                    .unwrap_or_else(|| ir_func.next_value())
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                let then_bb = ir_func.add_block("if.then");
                let merge_bb = ir_func.add_block("if.merge");
                let else_bb = if else_block.is_some() {
                    Some(ir_func.add_block("if.else"))
                } else {
                    None
                };

                let first_false_bb = if !elif_blocks.is_empty() {
                    ir_func.add_block("if.elif.0.cond")
                } else if let Some(else_id) = else_bb {
                    else_id
                } else {
                    merge_bb
                };

                let cond_value = self.lower_expression(condition, ir_func);
                self.builder
                    .build_cond_branch(ir_func, cond_value, then_bb, first_false_bb);

                let mut phi_inputs: Vec<(Value, usize)> = Vec::new();
                let mut merge_has_predecessor = first_false_bb == merge_bb;

                self.builder.set_current_block(then_bb);
                let (then_value, then_final_block, then_has_terminator) =
                    self.lower_branch_block_result(then_block, ir_func, then_bb);

                if let Some(value) = then_value {
                    if !then_has_terminator {
                        phi_inputs.push((value, then_final_block));
                    }
                }

                if !then_has_terminator {
                    self.builder.build_branch(ir_func, merge_bb);
                    merge_has_predecessor = true;
                }

                let mut current_false_block = first_false_bb;

                for (idx, (elif_condition, elif_body)) in elif_blocks.iter().enumerate() {
                    self.builder.set_current_block(current_false_block);
                    let cond_value = self.lower_expression(elif_condition, ir_func);

                    let elif_body_block = ir_func.add_block(&format!("if.elif.{}.body", idx));
                    let next_false_block = if idx + 1 < elif_blocks.len() {
                        ir_func.add_block(&format!("if.elif.{}.cond", idx + 1))
                    } else if let Some(else_id) = else_bb {
                        else_id
                    } else {
                        merge_bb
                    };

                    self.builder.build_cond_branch(
                        ir_func,
                        cond_value,
                        elif_body_block,
                        next_false_block,
                    );

                    if next_false_block == merge_bb {
                        merge_has_predecessor = true;
                    }

                    self.builder.set_current_block(elif_body_block);
                    let (elif_value, elif_final_block, elif_has_terminator) =
                        self.lower_branch_block_result(elif_body, ir_func, elif_body_block);

                    if let Some(value) = elif_value {
                        if !elif_has_terminator {
                            phi_inputs.push((value, elif_final_block));
                        }
                    }

                    if !elif_has_terminator {
                        self.builder.build_branch(ir_func, merge_bb);
                        merge_has_predecessor = true;
                    }

                    current_false_block = next_false_block;
                }

                if let Some(else_block_ast) = else_block {
                    self.builder.set_current_block(current_false_block);
                    let (else_value, else_final_block, else_has_terminator) = self
                        .lower_branch_block_result(else_block_ast, ir_func, current_false_block);

                    if let Some(value) = else_value {
                        if !else_has_terminator {
                            phi_inputs.push((value, else_final_block));
                        }
                    }

                    if !else_has_terminator {
                        self.builder.build_branch(ir_func, merge_bb);
                        merge_has_predecessor = true;
                    }
                } else if current_false_block != merge_bb {
                    self.builder.set_current_block(current_false_block);
                    self.builder.build_branch(ir_func, merge_bb);
                    merge_has_predecessor = true;
                }

                if merge_has_predecessor {
                    self.builder.set_current_block(merge_bb);
                    if phi_inputs.len() >= 2 {
                        self.builder.build_phi(ir_func, phi_inputs)
                    } else {
                        ir_func.next_value()
                    }
                } else {
                    // Merge block is unreachable (all branches return/tail-call);
                    // seal it with Unreachable so the IR verifier is happy.
                    self.builder.set_current_block(merge_bb);
                    self.builder.build_unreachable(ir_func);
                    ir_func.next_value()
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                // Unless is equivalent to: if (!condition) { then_block } else { else_block }
                let unless_then_bb = ir_func.add_block("unless.then");
                let unless_else_bb = ir_func.add_block("unless.else");
                let unless_merge_bb = ir_func.add_block("unless.merge");

                // Evaluate and negate condition
                let cond_value = self.lower_expression(condition, ir_func);
                let negated_cond = self.builder.build_not(ir_func, cond_value);

                self.builder.build_cond_branch(
                    ir_func,
                    negated_cond,
                    unless_then_bb,
                    unless_else_bb,
                );

                // Unless body (executes when condition is false)
                self.builder.set_current_block(unless_then_bb);
                let mut unless_value = None;
                self.lower_block(&then_block.statements, ir_func);
                if let Some(Statement {
                    kind: StatementKind::Expression(expr),
                    ..
                }) = then_block.statements.last()
                {
                    unless_value = Some(self.lower_expression(expr, ir_func));
                }
                let unless_then_final = self.builder.get_current_block().unwrap_or(unless_then_bb);

                // Only add branch if block doesn't have terminator
                if let Some(block) = ir_func.get_block_mut(unless_then_final) {
                    if block.terminator.is_none() {
                        self.builder.build_branch(ir_func, unless_merge_bb);
                    }
                }

                // Else branch (executes when condition is true)
                self.builder.set_current_block(unless_else_bb);
                let mut unless_else_value = None;
                if let Some(else_body) = else_block {
                    self.lower_block(&else_body.statements, ir_func);
                    if let Some(Statement {
                        kind: StatementKind::Expression(expr),
                        ..
                    }) = else_body.statements.last()
                    {
                        unless_else_value = Some(self.lower_expression(expr, ir_func));
                    }
                }
                let unless_else_final = self.builder.get_current_block().unwrap_or(unless_else_bb);

                // Check if else block has terminator
                let else_has_terminator = if let Some(block) = ir_func.get_block(unless_else_final)
                {
                    block.terminator.is_some()
                } else {
                    false
                };

                // Only add branch if block doesn't have terminator
                if !else_has_terminator {
                    if let Some(block) = ir_func.get_block_mut(unless_else_final) {
                        if block.terminator.is_none() {
                            self.builder.build_branch(ir_func, unless_merge_bb);
                        }
                    }
                }

                // Check if then block has terminator
                let then_has_terminator = if let Some(block) = ir_func.get_block(unless_then_final)
                {
                    block.terminator.is_some()
                } else {
                    false
                };

                // Only use merge block if at least one branch reaches it
                if !then_has_terminator || !else_has_terminator {
                    // Merge block with PHI node
                    self.builder.set_current_block(unless_merge_bb);

                    // If both branches produce values, create PHI node
                    if let (Some(then_val), Some(else_val)) = (unless_value, unless_else_value) {
                        self.builder.build_phi(
                            ir_func,
                            vec![(then_val, unless_then_final), (else_val, unless_else_final)],
                        )
                    } else {
                        // No value produced (void)
                        ir_func.next_value()
                    }
                } else {
                    // Both branches have terminators (returns), merge block is unreachable.
                    self.builder.set_current_block(unless_merge_bb);
                    self.builder.build_unreachable(ir_func);
                    ir_func.next_value()
                }
            }
            ExpressionKind::Grouping(inner) => self.lower_expression(inner, ir_func),
            ExpressionKind::ArrayLiteral { elements } => {
                // Alocar memória para o array
                let size = elements.len();
                if size == 0 {
                    // Array vazio — emitir uma constante inteira 0 como valor
                    // sentinel em vez de consumir um Value ID sem instrução associada.
                    return self.builder.build_const_int(ir_func, 0);
                }

                // Inferir o tipo dos elementos
                let elem_type = self.infer_array_element_type(elements);

                // Alocar espaço para o array no stack (tipo Array com tamanho)
                let array_type = IRType::Array {
                    element_type: Box::new(elem_type.clone()),
                    size,
                };
                let array_ptr = self.builder.build_alloca(ir_func, array_type);

                // Inicializar cada elemento
                for (i, elem_expr) in elements.iter().enumerate() {
                    let elem_value = self.lower_expression(elem_expr, ir_func);
                    let index_value = self.builder.build_const_int(ir_func, i as i64);
                    let elem_ptr = self.builder.build_getelementptr(
                        ir_func,
                        array_ptr,
                        index_value,
                        elem_type.clone(),
                    );
                    self.builder.build_store(ir_func, elem_ptr, elem_value);
                }

                // Retornar o ponteiro para o array
                array_ptr
            }
            ExpressionKind::IndexAccess { array, index } => {
                // Avaliar a expressão do array
                let array_ptr = self.lower_expression(array, ir_func);

                // Avaliar o índice
                let index_value = self.lower_expression(index, ir_func);

                // Calcular o endereço do elemento
                // Por simplicidade, assumir tipo Int
                let elem_type = IRType::Int;
                let elem_ptr =
                    self.builder
                        .build_getelementptr(ir_func, array_ptr, index_value, elem_type);

                // Carregar o valor do elemento
                self.builder.build_load(ir_func, elem_ptr)
            }
            ExpressionKind::TupleLiteral { elements } => {
                // Alocar memória para a tuple
                let size = elements.len();
                if size == 0 {
                    // Tuple vazia — emitir constante 0 como sentinel em vez de
                    // consumir um Value ID sem instrução.
                    return self.builder.build_const_int(ir_func, 0);
                }

                // Determinar os tipos dos elementos usando inferência
                let elem_types: Vec<IRType> = elements
                    .iter()
                    .map(|e| self.infer_expr_ir_type(e))
                    .collect();

                // Alocar espaço para a tuple no stack
                let tuple_type = IRType::Tuple {
                    elements: elem_types.clone(),
                };
                let tuple_ptr = self.builder.build_alloca(ir_func, tuple_type);

                // Inicializar cada elemento
                for (i, elem_expr) in elements.iter().enumerate() {
                    let elem_value = self.lower_expression(elem_expr, ir_func);
                    let index_value = self.builder.build_const_int(ir_func, i as i64);
                    let elem_ptr = self.builder.build_getelementptr(
                        ir_func,
                        tuple_ptr,
                        index_value,
                        elem_types[i].clone(),
                    );
                    self.builder.build_store(ir_func, elem_ptr, elem_value);
                }

                // Retornar o ponteiro para a tuple
                tuple_ptr
            }
            ExpressionKind::TupleAccess { tuple, index } => {
                // Avaliar a expressão da tuple
                let tuple_ptr = self.lower_expression(tuple, ir_func);

                // Calcular o endereço do elemento usando o índice constante
                let index_value = self.builder.build_const_int(ir_func, *index as i64);

                // Inferir o tipo do elemento da tuple
                let elem_type = if let ExpressionKind::TupleLiteral { elements } = &tuple.kind {
                    // Se é um literal, inferir diretamente
                    if *index < elements.len() {
                        self.infer_expr_ir_type(&elements[*index])
                    } else {
                        IRType::Int
                    }
                } else {
                    // Caso contrário, inferir o tipo da tuple inteira e extrair o elemento
                    match self.infer_expr_ir_type(tuple) {
                        IRType::Tuple { elements } if *index < elements.len() => {
                            elements[*index].clone()
                        }
                        _ => IRType::Int, // Fallback
                    }
                };

                let elem_ptr = self.builder.build_getelementptr(
                    ir_func,
                    tuple_ptr,
                    index_value,
                    elem_type.clone(),
                );

                // Carregar o valor do elemento
                self.builder.build_load_typed(ir_func, elem_ptr, elem_type)
            }
            ExpressionKind::StructLiteral {
                name,
                fields,
                type_args,
            } => {
                let (actual_name, field_defs) =
                    self.ensure_struct_definition(name, type_args.as_slice());

                // Criar tipo struct
                let struct_type = IRType::Struct {
                    name: actual_name.clone(),
                    fields: field_defs.clone(),
                };

                // Alocar espaço para o struct no stack
                let struct_ptr = self.builder.build_alloca(ir_func, struct_type);

                // Inicializar cada campo
                for (field_name, field_expr) in fields.iter() {
                    let field_value = self.lower_expression(field_expr, ir_func);

                    let (field_idx, field_type) = field_defs
                        .iter()
                        .enumerate()
                        .find(|(_, (fname, _))| fname == field_name)
                        .map(|(idx, (_, ty))| (idx, ty.clone()))
                        .unwrap_or_else(|| {
                            self.error(format!(
                                "Field '{}' not found in struct '{}' definition",
                                field_name, actual_name
                            ));
                            (0, IRType::Int)
                        });

                    let index_value = self.builder.build_const_int(ir_func, field_idx as i64);
                    let field_ptr = self.builder.build_getelementptr(
                        ir_func,
                        struct_ptr,
                        index_value,
                        field_type,
                    );

                    self.builder.build_store(ir_func, field_ptr, field_value);
                }

                // Retornar ponteiro para o struct
                struct_ptr
            }
            ExpressionKind::FieldAccess { object, field } => {
                // Se o objeto é um identificador, buscar no struct_var_map
                if let ExpressionKind::Identifier(name) = &object.kind {
                    if let Some((struct_ptr, struct_name)) = self.struct_var_map.get(name) {
                        // Buscar definição do struct
                        if let Some(field_defs) = self.struct_definitions.get(&struct_name) {
                            // Encontrar índice do campo
                            if let Some((field_idx, (_, field_type))) = field_defs
                                .iter()
                                .enumerate()
                                .find(|(_, (fname, _))| fname == field)
                            {
                                // GEP para o campo
                                let index_value =
                                    self.builder.build_const_int(ir_func, field_idx as i64);
                                let field_ptr = self.builder.build_getelementptr(
                                    ir_func,
                                    struct_ptr,
                                    index_value,
                                    field_type.clone(),
                                );

                                // Load do campo
                                return self.builder.build_load_typed(
                                    ir_func,
                                    field_ptr,
                                    field_type.clone(),
                                );
                            }
                        }
                    }
                }
                let object_ptr = self.lower_expression(object, ir_func);
                if let IRType::Struct {
                    fields: field_defs, ..
                } = self.infer_expr_ir_type(object)
                {
                    if let Some((field_idx, field_ty)) = field_defs
                        .into_iter()
                        .enumerate()
                        .find(|(_, (fname, _))| fname == field)
                        .map(|(idx, (_, ty))| (idx, ty))
                    {
                        let index_value = self.builder.build_const_int(ir_func, field_idx as i64);
                        let field_ptr = self.builder.build_getelementptr(
                            ir_func,
                            object_ptr,
                            index_value,
                            field_ty.clone(),
                        );
                        return self
                            .builder
                            .build_load_typed(ir_func, field_ptr, field_ty.clone());
                    }
                }

                ir_func.next_value()
            }
            ExpressionKind::EnumVariant {
                module_path: _,
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
            } => {
                // Handle `StructType::static_method(args)` — parsed as EnumVariant by the
                // parser but is actually a static/associated function call.
                if self.struct_definitions.contains_key(enum_name.as_str())
                    || self.generic_structs.contains_key(enum_name.as_str())
                {
                    let function_name = format!("{}_{}", enum_name, variant_name);
                    let mut call_args: Vec<Value> = Vec::new();
                    if let Some(data_exprs) = data {
                        for arg in data_exprs.iter() {
                            call_args.push(self.lower_expression(arg, ir_func));
                        }
                    } else if let Some(named_fields) = struct_data {
                        for (_, val_expr) in named_fields.iter() {
                            call_args.push(self.lower_expression(val_expr, ir_func));
                        }
                    }
                    return self
                        .builder
                        .build_call(ir_func, function_name, call_args, true)
                        .unwrap_or_else(|| self.builder.build_const_int(ir_func, 0));
                }

                // Handle qualified-path function calls: module::function(args)
                // The parser can't distinguish these from EnumVariant, so we detect
                // them here when enum_name is not a known local type.
                let is_known_type = self.enum_definitions.contains_key(enum_name.as_str())
                    || self.generic_enums.contains_key(enum_name.as_str());
                let looks_like_call = data.is_some() || struct_data.is_some();
                if !is_known_type && looks_like_call {
                    let callee = variant_name.clone();
                    if self.function_return_types.contains_key(&callee)
                        || self.generic_functions.contains_key(&callee)
                    {
                        let mut call_args: Vec<Value> = Vec::new();
                        if let Some(data_exprs) = data {
                            for arg in data_exprs.iter() {
                                call_args.push(self.lower_expression(arg, ir_func));
                            }
                        } else if let Some(named_fields) = struct_data {
                            for (_, val_expr) in named_fields.iter() {
                                call_args.push(self.lower_expression(val_expr, ir_func));
                            }
                        }
                        let final_name = if self.generic_functions.contains_key(&callee) {
                            let concrete_types =
                                self.infer_argument_types(data.as_deref().unwrap_or(&[]));
                            let request = MonomorphizationRequest {
                                generic_name: callee.clone(),
                                concrete_types,
                            };
                            let mangled = request.mangled_name();
                            if !self.generated_specializations.contains_key(&mangled) {
                                self.pending_specializations.push(request);
                            }
                            mangled
                        } else {
                            callee
                        };
                        return self
                            .builder
                            .build_call(ir_func, final_name, call_args, true)
                            .unwrap_or_else(|| ir_func.next_value());
                    }
                }

                let needs_refinement = type_args.is_empty()
                    || type_args
                        .iter()
                        .any(|ann| self.type_annotation_needs_refinement(ann));

                let inferred_args = if needs_refinement {
                    if let Some(data_exprs) = data {
                        self.infer_enum_type_args_from_data(enum_name, variant_name, data_exprs)
                            .or_else(|| self.default_type_args_for_enum(enum_name))
                    } else if let Some(named_fields) = struct_data {
                        self.infer_enum_type_args_from_named_fields(
                            enum_name,
                            variant_name,
                            named_fields,
                        )
                        .or_else(|| self.default_type_args_for_enum(enum_name))
                    } else {
                        self.default_type_args_for_enum(enum_name)
                    }
                } else {
                    None
                };

                let final_args: Vec<TypeAnnotation> = if let Some(mut args) = inferred_args {
                    // If some args are still "unknown", try to fill them from the function's
                    // declared return type annotation (e.g., fn -> Result<int, string> means
                    // both Result::Ok and Result::Err should use the same specialization).
                    if args
                        .iter()
                        .any(|a| self.type_annotation_needs_refinement(a))
                    {
                        if let Some(ret_ann) = self.current_function_return_annotation.clone() {
                            if let TypeAnnotationKind::Generic {
                                name: ret_name,
                                type_args: ret_args,
                            } = &ret_ann.kind
                            {
                                if ret_name == enum_name && ret_args.len() == args.len() {
                                    for (arg, ret_arg) in args.iter_mut().zip(ret_args.iter()) {
                                        if self.type_annotation_needs_refinement(arg) {
                                            *arg = ret_arg.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    args
                } else {
                    // Check the function return annotation as the primary source of type args
                    if let Some(ret_ann) = self.current_function_return_annotation.clone() {
                        if let TypeAnnotationKind::Generic {
                            name: ret_name,
                            type_args: ret_args,
                        } = &ret_ann.kind
                        {
                            if ret_name == enum_name {
                                ret_args.clone()
                            } else {
                                type_args.clone()
                            }
                        } else {
                            type_args.clone()
                        }
                    } else {
                        type_args.clone()
                    }
                };

                let (resolved_enum_name, variants) =
                    self.ensure_enum_definition(enum_name, final_args.as_slice());

                let data_values: Vec<Value> = if let Some(data_exprs) = data {
                    data_exprs
                        .iter()
                        .map(|expr| self.lower_expression(expr, ir_func))
                        .collect()
                } else if let Some(named_fields) = struct_data {
                    self.reorder_named_variant_exprs(
                        &resolved_enum_name,
                        variant_name,
                        named_fields,
                    )
                    .unwrap_or_default()
                    .into_iter()
                    .map(|expr| self.lower_expression(expr, ir_func))
                    .collect()
                } else {
                    Vec::new()
                };

                if !variants.is_empty() {
                    // Encontrar o variant
                    if let Some((_, tag, variant_data_types)) =
                        variants.iter().find(|(name, _, _)| name == variant_name)
                    {
                        // Se é unit variant, alocar um slot de 8 bytes só com o tag
                        // (mesma representação por ponteiro dos variants com dados)
                        if variant_data_types.is_none() {
                            let tag_val = self.builder.build_const_int(ir_func, *tag as i64);
                            let tag_alloc = self.builder.build_alloca(
                                ir_func,
                                IRType::Tuple {
                                    elements: vec![IRType::Int],
                                },
                            );
                            let zero = self.builder.build_const_int(ir_func, 0);
                            let tag_slot = self.builder.build_getelementptr(
                                ir_func,
                                tag_alloc,
                                zero,
                                IRType::Int,
                            );
                            self.builder.build_store(ir_func, tag_slot, tag_val);
                            return tag_alloc;
                        }

                        // Se é tuple variant, criar tupla (tag, data...)
                        if let Some(data_types) = variant_data_types {
                            let mut elements = Vec::new();

                            // Primeiro elemento: tag
                            elements.push(self.builder.build_const_int(ir_func, *tag as i64));

                            // Demais elementos: dados do variant
                            for value in &data_values {
                                elements.push(*value);
                            }

                            // Criar tipos da tupla
                            let mut element_types = vec![IRType::Int];
                            element_types.extend(data_types.clone());

                            let tuple_type = IRType::Tuple {
                                elements: element_types.clone(),
                            };

                            // Alocar tupla no stack
                            let tuple_ptr = self.builder.build_alloca(ir_func, tuple_type.clone());

                            // Store cada elemento
                            for (idx, elem_value) in elements.iter().enumerate() {
                                let index_value = self.builder.build_const_int(ir_func, idx as i64);
                                let elem_ptr = self.builder.build_getelementptr(
                                    ir_func,
                                    tuple_ptr,
                                    index_value,
                                    element_types[idx].clone(),
                                );
                                self.builder.build_store(ir_func, elem_ptr, *elem_value);
                            }

                            return tuple_ptr;
                        }

                        // Variant com dados mas sem argumentos fornecidos - erro
                        return self.builder.build_const_int(ir_func, *tag as i64);
                    }
                }

                // Enum ou variant não encontrado
                ir_func.next_value()
            }
            ExpressionKind::Match { scrutinee, arms } => {
                // Lower do valor sendo matcheado
                let scrutinee_value = self.lower_expression(scrutinee, ir_func);

                let scrutinee_type = self.infer_expr_ir_type(scrutinee);
                let scrutinee_enum_name = if let IRType::Enum { name, .. } = &scrutinee_type {
                    Some(name.clone())
                } else {
                    None
                };

                // Criar blocos para cada arm e um bloco de saída
                let exit_block = ir_func.add_block("match_exit");
                let mut arm_check_blocks = Vec::new();
                let mut arm_body_blocks = Vec::new();

                // Criar blocos para cada arm: um para checar pattern, outro para executar body
                for (idx, _) in arms.iter().enumerate() {
                    arm_check_blocks.push(ir_func.add_block(&format!("match_check_{}", idx)));
                    arm_body_blocks.push(ir_func.add_block(&format!("match_body_{}", idx)));
                }

                // Inferir tipo do resultado combinando os tipos de cada arm
                let mut result_type = if let Some(first_arm) = arms.first() {
                    self.infer_match_arm_type(
                        &first_arm.pattern,
                        &first_arm.body,
                        scrutinee_enum_name.as_deref(),
                        &scrutinee_type,
                    )
                } else {
                    IRType::Int
                };
                for arm in arms.iter().skip(1) {
                    let arm_type = self.infer_match_arm_type(
                        &arm.pattern,
                        &arm.body,
                        scrutinee_enum_name.as_deref(),
                        &scrutinee_type,
                    );
                    if let Some(merged) = self.merge_types(&result_type, &arm_type) {
                        result_type = merged;
                    }
                }
                let result_alloca = if result_type != IRType::Void {
                    Some(self.builder.build_alloca(ir_func, result_type.clone()))
                } else {
                    None
                };

                // Do bloco atual, fazer branch para o primeiro check
                self.builder.build_branch(ir_func, arm_check_blocks[0]);

                // Processar cada arm
                for (idx, arm) in arms.iter().enumerate() {
                    // Bloco de checagem do pattern
                    self.builder.set_current_block(arm_check_blocks[idx]);

                    let pattern_matches = self.lower_pattern_check(
                        &arm.pattern,
                        scrutinee_value,
                        scrutinee_enum_name.as_deref(),
                        Some(&scrutinee_type),
                        ir_func,
                    );

                    // Próximo bloco: ou próximo arm, ou exit se não houver mais arms
                    let next_check = if idx + 1 < arms.len() {
                        arm_check_blocks[idx + 1]
                    } else {
                        exit_block
                    };

                    // Se pattern match, ir para body; senão, próximo check
                    self.builder.build_cond_branch(
                        ir_func,
                        pattern_matches,
                        arm_body_blocks[idx],
                        next_check,
                    );

                    // Bloco de execução do body
                    self.builder.set_current_block(arm_body_blocks[idx]);

                    // Fazer bindings do pattern antes de executar body
                    self.value_map.push_scope();
                    self.variable_types.push_scope();
                    self.array_map.push_scope();
                    self.struct_var_map.push_scope();

                    self.lower_pattern_bindings(
                        &arm.pattern,
                        scrutinee_value,
                        scrutinee_enum_name.as_deref(),
                        Some(&scrutinee_type),
                        ir_func,
                    );

                    // Se o arm tem guard (p. ex. `Pattern if cond =>`), avaliar a condição
                    // e saltar para o próximo check se ela for falsa.
                    if let Some(guard_expr) = &arm.guard {
                        let guard_val = self.lower_expression(guard_expr, ir_func);
                        let guard_body_block =
                            ir_func.add_block(&format!("match_guard_body_{}", idx));
                        self.builder.build_cond_branch(
                            ir_func,
                            guard_val,
                            guard_body_block,
                            next_check,
                        );
                        self.builder.set_current_block(guard_body_block);
                    }

                    let body_value = self.lower_expression(&arm.body, ir_func);
                    // Só emitir store+branch se o arm não terminou com return explícito
                    let arm_final_block = self
                        .builder
                        .get_current_block()
                        .unwrap_or(arm_body_blocks[idx]);
                    let arm_terminated = ir_func
                        .get_block(arm_final_block)
                        .map(|b| b.terminator.is_some())
                        .unwrap_or(false);
                    if !arm_terminated {
                        if let Some(result_alloca) = result_alloca {
                            self.builder.build_store(ir_func, result_alloca, body_value);
                        }
                        self.builder.build_branch(ir_func, exit_block);
                    }

                    self.struct_var_map.pop_scope();
                    self.array_map.pop_scope();
                    self.variable_types.pop_scope();
                    self.value_map.pop_scope();
                }

                // Bloco de saída
                self.builder.set_current_block(exit_block);
                if let Some(result_alloca) = result_alloca {
                    self.builder
                        .build_load_typed(ir_func, result_alloca, result_type.clone())
                } else {
                    self.builder.build_const_int(ir_func, 0)
                }
            }
            ExpressionKind::MethodCall {
                object,
                method_name,
                arguments,
                type_name,
            } => {
                // Check if this is actually a qualified stdlib function call
                // like `std.string.len(x)` parsed as MethodCall { object: std.string, method: "len" }
                if let Some(mut obj_path) = self.resolve_call_path(object) {
                    obj_path.push(method_name.clone());
                    let desc_opt = lookup_std_host_function(&obj_path).or_else(|| {
                        // alias.func fallback (2-segment alias path)
                        if obj_path.len() == 2 {
                            let alias_key = &obj_path[0];
                            let func_name = &obj_path[1];
                            if let Some(full_prefix) = self.std_import_aliases.get(alias_key) {
                                if full_prefix.len() >= 2 {
                                    let module_prefix = &full_prefix[..full_prefix.len() - 1];
                                    let mut resolved = module_prefix.to_vec();
                                    resolved.push(func_name.clone());
                                    return lookup_std_host_function(&resolved);
                                }
                            }
                        }
                        None
                    });
                    if let Some(desc) = desc_opt {
                        let mut call_args = Vec::new();
                        for arg in arguments {
                            call_args.push(self.lower_expression(arg, ir_func));
                        }
                        let result = self.builder.build_host_call(
                            ir_func,
                            desc.runtime_name.to_string(),
                            call_args,
                            desc.returns_value,
                        );
                        return result.unwrap_or_else(|| self.builder.build_const_int(ir_func, 0));
                    }
                }

                // Lower method call to function call: obj.method(args) -> Type_method(obj, args)

                // 1. Lower o objeto (self será o primeiro argumento)
                let obj_value = self.lower_expression(object, ir_func);

                // 1a. If the object is a dyn Trait, dispatch via vtable.
                let obj_ir_type = self.infer_expr_ir_type(object);
                if let IRType::DynTrait { trait_name } = &obj_ir_type {
                    return self.lower_dyn_method_call(
                        obj_value,
                        trait_name.clone(),
                        method_name,
                        arguments,
                        ir_func,
                    );
                }

                // 2. Determinar o tipo do objeto
                let obj_type_name = if let Some(name) = type_name {
                    // Tipo já foi preenchido pelo semantic analyzer
                    name.clone()
                } else {
                    match self.infer_expr_ir_type(object) {
                        IRType::Struct { name, .. } => name,
                        IRType::Enum { name, .. } => name,
                        other => {
                            self.error(format!(
                                "Could not determine object type for method call '{method_name}' (inferred type: {:?})",
                                other
                            ));
                            String::new()
                        }
                    }
                };

                // 3. Construir nome da função: Type_method
                let function_name = format!("{}_{}", obj_type_name, method_name);

                // 4. Lower argumentos
                let mut call_args = vec![obj_value]; // self é o primeiro argumento
                for arg in arguments {
                    let arg_value = self.lower_expression(arg, ir_func);
                    call_args.push(arg_value);
                }

                // 5. Fazer a chamada de função
                // Assumir que retorna algo (se for void, será ignorado depois)
                self.builder
                    .build_call(ir_func, function_name, call_args, true)
                    .unwrap_or_else(|| self.builder.build_const_int(ir_func, 0))
            }
            ExpressionKind::CharLiteral(c) => self.builder.build_const_int(ir_func, *c as i64),
            ExpressionKind::FString(parts) => {
                // Lower each part to a string value:
                // - Literal parts: inline string literals (already String type)
                // - Interpolated parts: lower expression then convert to String via
                //   the appropriate runtime function (int_to_string / float_to_string /
                //   bool_to_string). String-typed expressions pass through unchanged.
                // All parts are then concatenated via spectra.std.string.concat.
                let string_parts: Vec<Value> = parts
                    .iter()
                    .map(|part| match part {
                        FStringPart::Literal(s) => self.lower_string_literal(s, ir_func),
                        FStringPart::Interpolated(expr) => {
                            let val = self.lower_expression(expr, ir_func);
                            let ty = self.infer_expr_ir_type(expr);
                            let conv = match ty {
                                IRType::String => None,
                                IRType::Float => Some("spectra.std.convert.float_to_string"),
                                IRType::Bool => Some("spectra.std.convert.bool_to_string"),
                                _ => Some("spectra.std.convert.int_to_string"),
                            };
                            if let Some(runtime_fn) = conv {
                                self.builder
                                    .build_host_call(
                                        ir_func,
                                        runtime_fn.to_string(),
                                        vec![val],
                                        true,
                                    )
                                    .unwrap_or_else(|| ir_func.next_value())
                            } else {
                                val
                            }
                        }
                    })
                    .collect();

                if string_parts.is_empty() {
                    return self.lower_string_literal("", ir_func);
                }
                let mut result = string_parts[0];
                for part in &string_parts[1..] {
                    result = self
                        .builder
                        .build_host_call(
                            ir_func,
                            "spectra.std.string.concat".to_string(),
                            vec![result, *part],
                            true,
                        )
                        .unwrap_or(result);
                }
                result
            }
            ExpressionKind::Try(inner) => {
                // Proper Result-propagation semantics:
                //   1. Evaluate the inner expression → Result pointer (tagged heap tuple)
                //   2. Load the tag from slot 0
                //   3. tag == 0  →  Ok: extract the payload from slot 1 and continue
                //      tag != 0  →  Err: early-return the Result pointer directly
                //        (the calling function is assumed to also return Result<_, E>)
                let result_ptr = self.lower_expression(inner, ir_func);

                // Load tag (first field of the tagged tuple)
                let zero_idx = self.builder.build_const_int(ir_func, 0);
                let tag_ptr =
                    self.builder
                        .build_getelementptr(ir_func, result_ptr, zero_idx, IRType::Int);
                let tag_val = self.builder.build_load(ir_func, tag_ptr);

                // is_ok = (tag == 0)
                let zero = self.builder.build_const_int(ir_func, 0);
                let is_ok = self.builder.build_eq(ir_func, tag_val, zero);

                let ok_block = ir_func.add_block("try.ok");
                let err_block = ir_func.add_block("try.err");
                self.builder
                    .build_cond_branch(ir_func, is_ok, ok_block, err_block);

                // Err branch: early return the error result pointer
                self.builder.set_current_block(err_block);
                self.builder.build_return(ir_func, Some(result_ptr));

                // Ok branch: extract the Ok payload from slot 1
                self.builder.set_current_block(ok_block);
                let one_idx = self.builder.build_const_int(ir_func, 1);
                let payload_ptr =
                    self.builder
                        .build_getelementptr(ir_func, result_ptr, one_idx, IRType::Int);
                self.builder.build_load(ir_func, payload_ptr)
            }
            ExpressionKind::Range { start, end, .. } => {
                // Range expressions used outside of for-loops (e.g., stored in a variable)
                // Lower both bounds; return the start value as a placeholder.
                // Range-based for-loops are handled specially in lower_statement.
                let _end_val = self.lower_expression(end, ir_func);
                self.lower_expression(start, ir_func)
            }
            ExpressionKind::Lambda { params, body } => {
                // Lower as a top-level IR function with a generated unique name.
                let lambda_name = format!("__lambda_{}", self.lambda_counter);
                self.lambda_counter += 1;

                let captures = self.collect_lambda_captures(params, body);
                let lambda_func = self.lower_lambda(lambda_name.clone(), &captures, params, body);
                self.pending_lambdas.push(lambda_func);

                self.build_closure_object(ir_func, lambda_name, &captures)
            }
            ExpressionKind::Block(block) => {
                let stmts = &block.statements;
                if stmts.is_empty() {
                    return self.builder.build_const_int(ir_func, 0);
                }

                for stmt in &stmts[..stmts.len() - 1] {
                    self.lower_statement(stmt, ir_func);
                }

                let last = &stmts[stmts.len() - 1];
                match &last.kind {
                    spectra_compiler::ast::StatementKind::Expression(expr) => {
                        self.lower_expression(expr, ir_func)
                    }
                    _ => {
                        self.lower_statement(last, ir_func);
                        self.builder.build_const_int(ir_func, 0)
                    }
                }
            }
            ExpressionKind::DifferentiableBlock(block) => {
                let stmts = &block.statements;
                let loss = if stmts.is_empty() {
                    self.builder.build_const_int(ir_func, 0)
                } else {
                    for stmt in &stmts[..stmts.len() - 1] {
                        self.lower_statement(stmt, ir_func);
                    }

                    let last = &stmts[stmts.len() - 1];
                    match &last.kind {
                        spectra_compiler::ast::StatementKind::Expression(expr) => {
                            self.lower_expression(expr, ir_func)
                        }
                        _ => {
                            self.lower_statement(last, ir_func);
                            self.builder.build_const_int(ir_func, 0)
                        }
                    }
                };
                self.builder.build_host_call(
                    ir_func,
                    "spectra.std.tensor.backward".to_string(),
                    vec![loss],
                    false,
                );
                loss
            }
            ExpressionKind::Cast {
                expr: inner,
                target_type,
            } => self.lower_cast_expression(inner, target_type, ir_func),
        }
    }

    /// Lower a `expr as TargetType` cast expression.
    fn lower_cast_expression(
        &mut self,
        inner: &Expression,
        target_type: &TypeAnnotation,
        ir_func: &mut IRFunction,
    ) -> Value {
        let from_ty = self.infer_expr_ir_type(inner);
        let to_ty = self.lower_type_annotation(target_type);
        let operand = self.lower_expression(inner, ir_func);

        // Special case: coerce struct to dyn Trait
        if let IRType::DynTrait { trait_name } = &to_ty.clone() {
            return self.lower_coerce_to_dyn(operand, &from_ty, trait_name, ir_func);
        }

        // If same type, just copy
        if from_ty == to_ty {
            return operand;
        }

        self.builder.build_cast(ir_func, operand, from_ty, to_ty)
    }

    /// Dispatch a method call via vtable for `dyn Trait` objects.
    /// The fat_ptr contains (data_ptr, vtable_ptr); we look up the method slot
    /// and emit a `CallIndirect`.
    fn lower_dyn_method_call(
        &mut self,
        fat_ptr: Value,
        trait_name: String,
        method_name: &str,
        arguments: &[Expression],
        ir_func: &mut IRFunction,
    ) -> Value {
        // Extract data_ptr and vtable_ptr from fat pointer
        let data_ptr = self.builder.build_load_dyn_data_ptr(ir_func, fat_ptr);
        let vtable_ptr = self.builder.build_load_dyn_vtable_ptr(ir_func, fat_ptr);

        // Determine slot index by looking up the trait's method order
        let slot_index = self
            .trait_method_order
            .get(&trait_name)
            .and_then(|methods| methods.iter().position(|m| m == method_name))
            .unwrap_or(0);

        // Load function pointer from vtable
        let fn_ptr = self
            .builder
            .build_load_vtable_slot(ir_func, vtable_ptr, slot_index);

        // Build argument list: data_ptr first, then the other args
        let mut call_args = vec![data_ptr];
        for arg in arguments {
            call_args.push(self.lower_expression(arg, ir_func));
        }

        let (sig_params, sig_return) = self
            .trait_method_signatures
            .get(&trait_name)
            .and_then(|methods| methods.get(method_name))
            .map(|(params, return_type)| {
                let mut signature_params = Vec::with_capacity(params.len() + 1);
                signature_params.push(IRType::Int); // lowered receiver pointer
                signature_params.extend(params.iter().cloned());
                (signature_params, return_type.clone())
            })
            .unwrap_or_else(|| {
                let signature_params: Vec<IRType> = std::iter::once(IRType::Int)
                    .chain(call_args.iter().skip(1).map(|_| IRType::Int))
                    .collect();
                (signature_params, IRType::Int)
            });

        self.builder
            .build_call_indirect(ir_func, fn_ptr, call_args, sig_params, sig_return)
            .unwrap_or_else(|| self.builder.build_const_int(ir_func, 0))
    }

    /// Build a fat pointer (data_ptr, vtable_ptr) for coercing a concrete struct to `dyn Trait`.
    fn lower_coerce_to_dyn(
        &mut self,
        data_ptr: Value,
        from_ty: &IRType,
        trait_name: &str,
        ir_func: &mut IRFunction,
    ) -> Value {
        let vtable_ptr = if let IRType::Struct { name, .. } = from_ty {
            let methods = self
                .trait_method_order
                .get(trait_name)
                .cloned()
                .unwrap_or_default();

            let vtable_storage = self.builder.build_alloca(
                ir_func,
                IRType::Array {
                    element_type: Box::new(IRType::Int),
                    size: methods.len().max(1),
                },
            );

            for (slot, method_name) in methods.iter().enumerate() {
                let fn_name = format!("{}_{}", name, method_name);
                let fn_addr = self.builder.build_func_addr(ir_func, fn_name);
                let slot_index = self.builder.build_const_int(ir_func, slot as i64);
                let slot_ptr = self.builder.build_getelementptr(
                    ir_func,
                    vtable_storage,
                    slot_index,
                    IRType::Int,
                );
                self.builder.build_store(ir_func, slot_ptr, fn_addr);
            }

            vtable_storage
        } else {
            self.builder.build_const_int(ir_func, 0)
        };
        self.builder
            .build_make_dyn_fat_ptr(ir_func, data_ptr, vtable_ptr)
    }

    fn lower_string_literal(&mut self, literal: &str, ir_func: &mut IRFunction) -> Value {
        // Allocate buffer with trailing null terminator
        let bytes = literal.as_bytes();
        let total_size = bytes.len() + 1; // +1 for '\0'
        let array_type = IRType::Array {
            element_type: Box::new(IRType::Int),
            size: total_size,
        };

        let buffer_ptr = self.builder.build_alloca(ir_func, array_type);

        // Populate buffer with literal contents
        for (idx, byte) in bytes.iter().enumerate() {
            let index = self.builder.build_const_int(ir_func, idx as i64);
            let slot_ptr =
                self.builder
                    .build_getelementptr(ir_func, buffer_ptr, index, IRType::Int);
            let value = self.builder.build_const_int(ir_func, *byte as i64);
            self.builder.build_store(ir_func, slot_ptr, value);
        }

        // Null terminator at the end
        let terminator_index = self.builder.build_const_int(ir_func, bytes.len() as i64);
        let terminator_ptr =
            self.builder
                .build_getelementptr(ir_func, buffer_ptr, terminator_index, IRType::Int);
        let zero = self.builder.build_const_int(ir_func, 0);
        self.builder.build_store(ir_func, terminator_ptr, zero);

        buffer_ptr
    }

    fn lower_pattern_check(
        &mut self,
        pattern: &spectra_compiler::ast::Pattern,
        scrutinee: Value,
        scrutinee_enum: Option<&str>,
        scrutinee_type: Option<&IRType>,
        ir_func: &mut IRFunction,
    ) -> Value {
        use spectra_compiler::ast::Pattern;

        match pattern {
            Pattern::Wildcard => {
                // Wildcard sempre match
                self.builder.build_const_int(ir_func, 1)
            }
            Pattern::Identifier(_name) => {
                // Binding sempre match
                self.builder.build_const_int(ir_func, 1)
            }
            Pattern::Literal(expr) => {
                // Comparar scrutinee com o valor literal
                let literal_value = self.lower_expression(expr, ir_func);
                self.builder.build_eq(ir_func, scrutinee, literal_value)
            }
            Pattern::Tuple(elements) => {
                if let Some(IRType::Tuple {
                    elements: tuple_types,
                }) = scrutinee_type
                {
                    let mut result = self.builder.build_const_int(ir_func, 1);
                    for (idx, pattern) in elements.iter().enumerate() {
                        if let Some(field_ty) = tuple_types.get(idx) {
                            let index_value = self.builder.build_const_int(ir_func, idx as i64);
                            let field_ptr = self.builder.build_getelementptr(
                                ir_func,
                                scrutinee,
                                index_value,
                                field_ty.clone(),
                            );
                            let field_value =
                                self.builder
                                    .build_load_typed(ir_func, field_ptr, field_ty.clone());
                            let sub_match = self.lower_pattern_check(
                                pattern,
                                field_value,
                                None,
                                Some(field_ty),
                                ir_func,
                            );
                            result = self.builder.build_and(ir_func, result, sub_match);
                        }
                    }
                    result
                } else {
                    self.builder.build_const_int(ir_func, 0)
                }
            }
            Pattern::Struct { fields, .. } => {
                if let Some(IRType::Struct {
                    fields: struct_fields,
                    ..
                }) = scrutinee_type
                {
                    let field_map: HashMap<String, (usize, IRType)> = struct_fields
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(|(idx, (name, ty))| (name, (idx, ty)))
                        .collect();
                    let mut result = self.builder.build_const_int(ir_func, 1);
                    for (field_name, pattern) in fields {
                        if let Some((idx, field_ty)) = field_map.get(field_name) {
                            let index_value = self.builder.build_const_int(ir_func, *idx as i64);
                            let field_ptr = self.builder.build_getelementptr(
                                ir_func,
                                scrutinee,
                                index_value,
                                field_ty.clone(),
                            );
                            let field_value =
                                self.builder
                                    .build_load_typed(ir_func, field_ptr, field_ty.clone());
                            let sub_match = self.lower_pattern_check(
                                pattern,
                                field_value,
                                None,
                                Some(field_ty),
                                ir_func,
                            );
                            result = self.builder.build_and(ir_func, result, sub_match);
                        }
                    }
                    result
                } else {
                    self.builder.build_const_int(ir_func, 0)
                }
            }
            Pattern::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data: _,
                struct_data: _,
                ..
            } => {
                let mut variants = scrutinee_enum
                    .and_then(|name| self.enum_definitions.get(name).cloned())
                    .or_else(|| {
                        if let Some(IRType::Enum { name, .. }) = scrutinee_type {
                            self.enum_definitions.get(name).cloned()
                        } else {
                            None
                        }
                    })
                    .or_else(|| self.enum_definitions.get(enum_name).cloned());

                if variants.is_none() && !type_args.is_empty() {
                    let (_, specialized) =
                        self.ensure_enum_definition(enum_name, type_args.as_slice());
                    variants = Some(specialized);
                }

                if let Some(variants) = variants {
                    if let Some((_, expected_tag, variant_types)) =
                        variants.iter().find(|(name, _, _)| name == variant_name)
                    {
                        // Para qualquer variant (unit ou com dados), extrair tag do ponteiro
                        let zero_index = self.builder.build_const_int(ir_func, 0);
                        let tag_ptr = self.builder.build_getelementptr(
                            ir_func,
                            scrutinee,
                            zero_index,
                            IRType::Int,
                        );
                        let tag_value = self.builder.build_load(ir_func, tag_ptr);
                        let expected_tag_value =
                            self.builder.build_const_int(ir_func, *expected_tag as i64);
                        let _ = variant_types; // mantido para futuros guards
                        return self
                            .builder
                            .build_eq(ir_func, tag_value, expected_tag_value);
                    }
                }
                // Fallback: sempre false
                self.builder.build_const_int(ir_func, 0)
            }
            Pattern::Or(patterns) => {
                let mut branches = patterns.iter();
                if let Some(first) = branches.next() {
                    let mut result = self.lower_pattern_check(
                        first,
                        scrutinee,
                        scrutinee_enum,
                        scrutinee_type,
                        ir_func,
                    );
                    for branch in branches {
                        let next = self.lower_pattern_check(
                            branch,
                            scrutinee,
                            scrutinee_enum,
                            scrutinee_type,
                            ir_func,
                        );
                        result = self.builder.build_or(ir_func, result, next);
                    }
                    result
                } else {
                    self.builder.build_const_int(ir_func, 0)
                }
            }
        }
    }

    /// Extrai valores do scrutinee e cria bindings locais de acordo com o pattern
    fn lower_pattern_bindings(
        &mut self,
        pattern: &spectra_compiler::ast::Pattern,
        scrutinee: Value,
        scrutinee_enum: Option<&str>,
        scrutinee_type: Option<&IRType>,
        ir_func: &mut IRFunction,
    ) {
        use spectra_compiler::ast::Pattern;

        match pattern {
            Pattern::Wildcard => {
                // Wildcard não cria bindings
            }
            Pattern::Identifier(name) => {
                // Criar variável local para o identifier binding
                // Usar value_map (valores diretos, não precisam de alloca/load)
                self.value_map.insert(name.clone(), scrutinee);
                if let Some(ty) = scrutinee_type {
                    self.variable_types.insert(name.clone(), ty.clone());
                }
            }
            Pattern::Literal(_) => {
                // Literal não cria bindings
            }
            Pattern::Tuple(elements) => {
                if let Some(IRType::Tuple {
                    elements: tuple_types,
                }) = scrutinee_type
                {
                    for (idx, pattern) in elements.iter().enumerate() {
                        if let Some(field_ty) = tuple_types.get(idx) {
                            let index_value = self.builder.build_const_int(ir_func, idx as i64);
                            let field_ptr = self.builder.build_getelementptr(
                                ir_func,
                                scrutinee,
                                index_value,
                                field_ty.clone(),
                            );
                            let field_value =
                                self.builder
                                    .build_load_typed(ir_func, field_ptr, field_ty.clone());
                            self.lower_pattern_bindings(
                                pattern,
                                field_value,
                                None,
                                Some(field_ty),
                                ir_func,
                            );
                        }
                    }
                }
            }
            Pattern::Struct { fields, .. } => {
                if let Some(IRType::Struct {
                    fields: struct_fields,
                    ..
                }) = scrutinee_type
                {
                    let field_map: HashMap<String, (usize, IRType)> = struct_fields
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(|(idx, (name, ty))| (name, (idx, ty)))
                        .collect();
                    for (field_name, pattern) in fields {
                        if let Some((idx, field_ty)) = field_map.get(field_name) {
                            let index_value = self.builder.build_const_int(ir_func, *idx as i64);
                            let field_ptr = self.builder.build_getelementptr(
                                ir_func,
                                scrutinee,
                                index_value,
                                field_ty.clone(),
                            );
                            let field_value =
                                self.builder
                                    .build_load_typed(ir_func, field_ptr, field_ty.clone());
                            self.lower_pattern_bindings(
                                pattern,
                                field_value,
                                None,
                                Some(field_ty),
                                ir_func,
                            );
                        }
                    }
                }
            }
            Pattern::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
                ..
            } => {
                // Se há patterns de data, extrair valores e fazer binding recursivo
                let ordered_patterns: Vec<&spectra_compiler::ast::Pattern> =
                    if let Some(patterns) = data {
                        patterns.iter().collect()
                    } else if let Some(named_patterns) = struct_data {
                        self.reorder_named_variant_patterns(
                            scrutinee_enum.unwrap_or(enum_name),
                            variant_name,
                            named_patterns,
                        )
                        .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                if !ordered_patterns.is_empty() {
                    let mut variants = scrutinee_enum
                        .and_then(|name| self.enum_definitions.get(name).cloned())
                        .or_else(|| {
                            if let Some(IRType::Enum { name, .. }) = scrutinee_type {
                                self.enum_definitions.get(name).cloned()
                            } else {
                                None
                            }
                        })
                        .or_else(|| self.enum_variants_from_ir_type(scrutinee_type))
                        .or_else(|| self.enum_definitions.get(enum_name).cloned());

                    if variants.is_none() && !type_args.is_empty() {
                        let (_, specialized) =
                            self.ensure_enum_definition(enum_name, type_args.as_slice());
                        variants = Some(specialized);
                    }

                    if let Some(variants) = variants {
                        if let Some((_, _tag, variant_types)) =
                            variants.iter().find(|(name, _, _)| name == variant_name)
                        {
                            if let Some(types) = variant_types {
                                // Para cada pattern de data, extrair o valor correspondente
                                for (idx, sub_pattern) in ordered_patterns.iter().enumerate() {
                                    if let Some(sub_type) = types.get(idx) {
                                        // Extrair elemento idx+1 da tuple (idx 0 é o tag)
                                        let index_value =
                                            self.builder.build_const_int(ir_func, (idx + 1) as i64);
                                        let element_ptr = self.builder.build_getelementptr(
                                            ir_func,
                                            scrutinee,
                                            index_value,
                                            sub_type.clone(),
                                        );
                                        let element_value = self.builder.build_load_typed(
                                            ir_func,
                                            element_ptr,
                                            sub_type.clone(),
                                        );

                                        let next_enum = match sub_type {
                                            IRType::Enum { name, .. } => Some(name.clone()),
                                            _ => None,
                                        };

                                        // Recursivamente fazer binding do sub-pattern
                                        self.lower_pattern_bindings(
                                            sub_pattern,
                                            element_value,
                                            next_enum.as_deref(),
                                            Some(sub_type),
                                            ir_func,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Pattern::Or(patterns) => {
                if let Some(first) = patterns.first() {
                    self.lower_pattern_bindings(
                        first,
                        scrutinee,
                        scrutinee_enum,
                        scrutinee_type,
                        ir_func,
                    );
                }
            }
        }
    }

    fn lower_type_annotation_with_map(
        &self,
        type_ann: &TypeAnnotation,
        substitutions: &HashMap<String, IRType>,
    ) -> IRType {
        use spectra_compiler::ast::TypeAnnotationKind;

        match &type_ann.kind {
            TypeAnnotationKind::Simple { segments } => {
                if segments.is_empty() {
                    return IRType::Void;
                }

                // Check if this is a type parameter that needs substitution
                let type_name = segments[0].as_str();
                if let Some(concrete_type) = substitutions.get(type_name) {
                    return concrete_type.clone();
                }

                match type_name {
                    "int" | "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32"
                    | "u64" | "usize" => IRType::Int,
                    "float" | "f16" | "bf16" | "f32" | "f64" => IRType::Float,
                    "bool" => IRType::Bool,
                    "string" => IRType::String,
                    "char" => IRType::Char,
                    _ => {
                        // Check if this is a struct type
                        if let Some(fields) = self.struct_definitions.get(type_name) {
                            IRType::Struct {
                                name: type_name.to_string(),
                                fields: fields.clone(),
                            }
                        } else if let Some(variants) = self.enum_definitions.get(type_name) {
                            let simplified = variants
                                .iter()
                                .map(|(variant_name, _, data)| (variant_name.clone(), data.clone()))
                                .collect();
                            IRType::Enum {
                                name: type_name.to_string(),
                                variants: simplified,
                            }
                        } else if let Some(generic_enum) = self.generic_enums.get(type_name) {
                            let simplified = generic_enum
                                .variants
                                .iter()
                                .map(|variant| {
                                    let data_types = variant.data.as_ref().map(|types| {
                                        types
                                            .iter()
                                            .map(|ann| self.lower_type_annotation(ann))
                                            .collect::<Vec<_>>()
                                    });
                                    (variant.name.clone(), data_types)
                                })
                                .collect();
                            IRType::Enum {
                                name: type_name.to_string(),
                                variants: simplified,
                            }
                        } else {
                            IRType::Void
                        }
                    }
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                let ir_elements: Vec<IRType> = elements
                    .iter()
                    .map(|elem_ann| self.lower_type_annotation_with_map(elem_ann, substitutions))
                    .collect();
                IRType::Tuple {
                    elements: ir_elements,
                }
            }
            TypeAnnotationKind::Function {
                params,
                return_type,
            } => {
                let ir_params = params
                    .iter()
                    .map(|ann| self.lower_type_annotation_with_map(ann, substitutions))
                    .collect();
                let ir_return_type =
                    Box::new(self.lower_type_annotation_with_map(return_type, substitutions));
                IRType::Function {
                    params: ir_params,
                    return_type: ir_return_type,
                }
            }
            TypeAnnotationKind::Generic { name, type_args } => {
                if name == "Tensor" && !type_args.is_empty() {
                    let dtype = self.lower_type_annotation_with_map(&type_args[0], substitutions);
                    let meta = tensor_metadata(&type_args[1..]);
                    return IRType::Tensor {
                        dtype: Box::new(dtype),
                        rank: meta.rank,
                        dims: meta.dims,
                        layout: meta.layout,
                        device: meta.device,
                    };
                }

                // Resolve to the monomorphized enum type.
                // First, try the already-specialized version (e.g., "Option_int").
                let type_names: Vec<String> = type_args
                    .iter()
                    .map(|ty| self.type_annotation_to_string(ty))
                    .collect();
                let mangled = format!("{}_{}", name, type_names.join("_"));
                if let Some(variants) = self.enum_definitions.get(&mangled) {
                    let simplified = variants
                        .iter()
                        .map(|(vn, _, data)| (vn.clone(), data.clone()))
                        .collect();
                    return IRType::Enum {
                        name: mangled,
                        variants: simplified,
                    };
                }
                // Specialization not yet registered — use the generic enum with substituted
                // type args so the caller at least gets Enum { name: "Option_int", ... }.
                // The actual specialization will be triggered by ensure_enum_definition
                // during lowering of the call site.
                if let Some(generic_enum) = self.generic_enums.get(name.as_str()) {
                    let mut type_map: HashMap<String, TypeAnnotation> = HashMap::new();
                    for (param, arg) in generic_enum.type_params.iter().zip(type_args.iter()) {
                        type_map.insert(param.name.clone(), arg.clone());
                    }
                    let simplified: Vec<(String, Option<Vec<IRType>>)> = generic_enum
                        .variants
                        .iter()
                        .map(|v| {
                            let data = v.data.as_ref().map(|types| {
                                types
                                    .iter()
                                    .map(|ty| {
                                        let subst = self.substitute_type(ty, &type_map);
                                        self.lower_type_annotation_with_map(&subst, substitutions)
                                    })
                                    .collect()
                            });
                            (v.name.clone(), data)
                        })
                        .collect();
                    return IRType::Enum {
                        name: mangled,
                        variants: simplified,
                    };
                }
                // Ultimate fallback: treat as simple named type
                self.lower_type_annotation_with_map(
                    &TypeAnnotation {
                        kind: TypeAnnotationKind::Simple {
                            segments: vec![name.clone()],
                        },
                        span: spectra_compiler::span::Span::dummy(),
                    },
                    substitutions,
                )
            }
            TypeAnnotationKind::DynTrait { trait_name } => IRType::DynTrait {
                trait_name: trait_name.clone(),
            },
        }
    }

    fn lower_type_annotation(&self, type_ann: &TypeAnnotation) -> IRType {
        self.lower_type_annotation_with_map(type_ann, &self.type_substitution_map)
    }

    #[allow(dead_code)]
    fn lower_type(&self, ast_type: &ASTType) -> IRType {
        match ast_type {
            ASTType::Int => IRType::Int,
            ASTType::Float => IRType::Float,
            ASTType::Bool => IRType::Bool,
            ASTType::String => IRType::String,
            ASTType::Char => IRType::Char,
            ASTType::Unit => IRType::Void,
            ASTType::Unknown => IRType::Void,
            ASTType::Array { element_type, .. } => {
                // Arrays são representados como ponteiros no IR
                IRType::Pointer(Box::new(self.lower_type(element_type)))
            }
            ASTType::Tuple { elements } => {
                // Converter cada tipo do elemento
                let ir_elements: Vec<IRType> = elements
                    .iter()
                    .map(|elem_type| self.lower_type(elem_type))
                    .collect();
                IRType::Tuple {
                    elements: ir_elements,
                }
            }
            ASTType::Struct { name: _ } => {
                // Structs são representados como ponteiros
                IRType::Pointer(Box::new(IRType::Void))
            }
            ASTType::Enum { name } => {
                // Enums são representados como tagged unions
                // Para simplificar, vamos representar como uma tupla ou int
                // dependendo se tem dados ou não
                if let Some(variants) = self.enum_definitions.get(name) {
                    // Se todos os variants são unit, usar int
                    let all_unit = variants.iter().all(|(_, _, data)| data.is_none());
                    if all_unit {
                        IRType::Int
                    } else {
                        // Se algum tem dados, precisa de tupla dinâmica
                        // Por simplificação, usar ponteiro genérico
                        IRType::Pointer(Box::new(IRType::Void))
                    }
                } else {
                    // Enum não encontrado, usar int como fallback
                    IRType::Int
                }
            }
            ASTType::TypeParameter { name: _ } => {
                // Type parameters são resolvidos via monomorphization
                // Por enquanto, tratar como ponteiro genérico
                IRType::Pointer(Box::new(IRType::Void))
            }
            ASTType::SelfType => {
                // Self type é resolvido para o tipo concreto do impl block
                // Por enquanto, tratar como ponteiro genérico (será resolvido no contexto)
                IRType::Pointer(Box::new(IRType::Void))
            }
            ASTType::Fn {
                params,
                return_type,
            } => {
                let ir_params = params.iter().map(|t| self.lower_type(t)).collect();
                let ir_return = Box::new(self.lower_type(return_type));
                IRType::Function {
                    params: ir_params,
                    return_type: ir_return,
                }
            }
            ASTType::Tensor {
                dtype,
                rank,
                dims,
                layout,
                device,
            } => IRType::Tensor {
                dtype: Box::new(self.lower_type(dtype)),
                rank: *rank,
                dims: dims.clone(),
                layout: layout.clone(),
                device: device.clone(),
            },
            ASTType::DynTrait { trait_name } => IRType::DynTrait {
                trait_name: trait_name.clone(),
            },
        }
    }

    /// Convert TypeAnnotation to string for name mangling
    fn type_annotation_to_string(&self, ty: &TypeAnnotation) -> String {
        match &ty.kind {
            TypeAnnotationKind::Simple { segments } => segments.join("::"),
            TypeAnnotationKind::Tuple { elements } => {
                let element_strs: Vec<String> = elements
                    .iter()
                    .map(|el| self.type_annotation_to_string(el))
                    .collect();
                format!("tuple_{}", element_strs.join("_"))
            }
            TypeAnnotationKind::Function { .. } => "function".to_string(),
            TypeAnnotationKind::Generic { name, type_args } => {
                let arg_strs: Vec<String> = type_args
                    .iter()
                    .map(|a| self.type_annotation_to_string(a))
                    .collect();
                format!("{}_{}", name, arg_strs.join("_"))
            }
            TypeAnnotationKind::DynTrait { trait_name } => format!("dyn_{}", trait_name),
        }
    }

    /// Specialize a generic struct with concrete type arguments
    fn specialize_struct(
        &mut self,
        generic: &ASTStruct,
        type_args: &[TypeAnnotation],
        mangled_name: &str,
    ) {
        // Create type substitution map: T -> int, U -> float, etc.
        let mut type_map: HashMap<String, TypeAnnotation> = HashMap::new();

        if generic.type_params.len() != type_args.len() {
            self.error(format!(
                "Type argument count mismatch for struct '{}': expected {}, got {}",
                generic.name,
                generic.type_params.len(),
                type_args.len()
            ));
            return;
        }

        for (param, arg) in generic.type_params.iter().zip(type_args.iter()) {
            type_map.insert(param.name.clone(), arg.clone());
        }

        // Substitute types in fields
        let specialized_fields: Vec<(String, IRType)> = generic
            .fields
            .iter()
            .map(|field| {
                let substituted_type = self.substitute_type(&field.ty, &type_map);
                let ir_type = self.lower_type_annotation(&substituted_type);
                (field.name.clone(), ir_type)
            })
            .collect();

        // Store specialized struct definition
        self.struct_definitions
            .insert(mangled_name.to_string(), specialized_fields);

        // specialized struct
    }

    /// Specialize a generic enum with concrete type arguments
    fn specialize_enum(
        &mut self,
        generic: &ASTEnum,
        type_args: &[TypeAnnotation],
        mangled_name: &str,
    ) {
        // Create type substitution map: T -> int, U -> float, etc.
        let mut type_map: HashMap<String, TypeAnnotation> = HashMap::new();

        if generic.type_params.len() != type_args.len() {
            self.error(format!(
                "Type argument count mismatch for enum '{}': expected {}, got {}",
                generic.name,
                generic.type_params.len(),
                type_args.len()
            ));
            return;
        }

        for (param, arg) in generic.type_params.iter().zip(type_args.iter()) {
            type_map.insert(param.name.clone(), arg.clone());
        }

        // Substitute types in variants
        let mut field_names = HashMap::new();
        let specialized_variants: Vec<(String, usize, Option<Vec<IRType>>)> = generic
            .variants
            .iter()
            .enumerate()
            .map(|(tag, variant)| {
                let variant_name = variant.name.clone();

                // Substitute types in variant data if present
                let variant_types = if let Some(ref data_types) = variant.data {
                    let substituted: Vec<IRType> = data_types
                        .iter()
                        .map(|ty| {
                            let substituted_type = self.substitute_type(ty, &type_map);
                            self.lower_type_annotation(&substituted_type)
                        })
                        .collect();
                    Some(substituted)
                } else if let Some(ref fields) = variant.struct_data {
                    field_names.insert(
                        variant_name.clone(),
                        fields.iter().map(|(name, _)| name.clone()).collect(),
                    );
                    let substituted: Vec<IRType> = fields
                        .iter()
                        .map(|(_, ty)| {
                            let substituted_type = self.substitute_type(ty, &type_map);
                            self.lower_type_annotation(&substituted_type)
                        })
                        .collect();
                    Some(substituted)
                } else {
                    None
                };

                (variant_name, tag, variant_types)
            })
            .collect();

        // Store specialized enum definition
        self.enum_definitions
            .insert(mangled_name.to_string(), specialized_variants);
        if !field_names.is_empty() {
            self.enum_variant_field_names
                .insert(mangled_name.to_string(), field_names);
        }

        // specialized enum
    }

    /// Substitute type parameters in a type annotation
    fn substitute_type(
        &self,
        ty: &TypeAnnotation,
        type_map: &HashMap<String, TypeAnnotation>,
    ) -> TypeAnnotation {
        let substituted_kind = match &ty.kind {
            TypeAnnotationKind::Simple { segments } => {
                // Check if this is a type parameter (single segment)
                if segments.len() == 1 {
                    if let Some(concrete) = type_map.get(&segments[0]) {
                        return concrete.clone();
                    }
                }
                TypeAnnotationKind::Simple {
                    segments: segments.clone(),
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                let subst_elements = elements
                    .iter()
                    .map(|el| self.substitute_type(el, type_map))
                    .collect();
                TypeAnnotationKind::Tuple {
                    elements: subst_elements,
                }
            }
            TypeAnnotationKind::Function {
                params,
                return_type,
            } => {
                let subst_params = params
                    .iter()
                    .map(|el| self.substitute_type(el, type_map))
                    .collect();
                let subst_ret = Box::new(self.substitute_type(return_type, type_map));
                TypeAnnotationKind::Function {
                    params: subst_params,
                    return_type: subst_ret,
                }
            }
            TypeAnnotationKind::Generic { name, type_args } => {
                let subst_args = type_args
                    .iter()
                    .map(|el| self.substitute_type(el, type_map))
                    .collect();
                TypeAnnotationKind::Generic {
                    name: name.clone(),
                    type_args: subst_args,
                }
            }
            TypeAnnotationKind::DynTrait { trait_name } => TypeAnnotationKind::DynTrait {
                trait_name: trait_name.clone(),
            },
        };

        TypeAnnotation {
            kind: substituted_kind,
            span: ty.span,
        }
    }
}

fn lookup_std_host_function(path: &[String]) -> Option<HostFunctionDescriptor> {
    match path {
        [] => None,
        [first, ..] if first != "std" => None,
        [_, module, function] => match (module.as_str(), function.as_str()) {
            ("math", "abs") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.abs",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("math", "min") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.min",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("math", "max") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.max",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("math", "clamp") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.clamp",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("math", "sqrt_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.sqrt_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "pow_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.pow_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "floor_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.floor_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "ceil_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.ceil_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "round_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.round_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "sin_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.sin_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "cos_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.cos_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "tan_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.tan_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "log_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.log_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "log2_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.log2_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "log10_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.log10_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "atan2_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.atan2_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "pi") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.pi",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "e_const") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.e_const",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "sign") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.sign",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("math", "abs_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.abs_f",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("math", "gcd") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.gcd",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("math", "lcm") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.lcm",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("math", "is_nan_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.is_nan_f",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("math", "is_infinite_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.math.is_infinite_f",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("io", "print") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.print",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("io", "println") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.println",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("io", "eprint") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.eprint",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("io", "eprintln") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.eprintln",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("io", "flush") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.flush",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("io", "read_line") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.read_line",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("collections", "list_new") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_new",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_push") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_push",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_len") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_len",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_get") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_get",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_set") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_set",
                return_type: IRType::Void,
                returns_value: false,
            }),
            ("collections", "list_contains") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_contains",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_clear") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_clear",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_free") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_free",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_free_all") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_free_all",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("tensor", "vector_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.tensor.full_f",
                return_type: IRType::Tensor {
                    dtype: Box::new(IRType::Float),
                    rank: Some(1),
                    dims: None,
                    layout: None,
                    device: None,
                },
                returns_value: true,
            }),
            ("tensor", "matrix_f") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.tensor.full2_f",
                return_type: IRType::Tensor {
                    dtype: Box::new(IRType::Float),
                    rank: Some(2),
                    dims: None,
                    layout: None,
                    device: None,
                },
                returns_value: true,
            }),
            ("tensor", "zeros") => Some(host_int("spectra.std.tensor.zeros")),
            ("tensor", "ones") => Some(host_int("spectra.std.tensor.ones")),
            ("tensor", "full") => Some(host_int("spectra.std.tensor.full")),
            ("tensor", "full_f") => Some(host_int("spectra.std.tensor.full_f")),
            ("tensor", "arange") => Some(host_int("spectra.std.tensor.arange")),
            ("tensor", "zeros2") => Some(host_int("spectra.std.tensor.zeros2")),
            ("tensor", "ones2") => Some(host_int("spectra.std.tensor.ones2")),
            ("tensor", "full2") => Some(host_int("spectra.std.tensor.full2")),
            ("tensor", "full2_f") => Some(host_int("spectra.std.tensor.full2_f")),
            ("tensor", "len") => Some(host_int("spectra.std.tensor.len")),
            ("tensor", "rank") => Some(host_int("spectra.std.tensor.rank")),
            ("tensor", "dim") => Some(host_int("spectra.std.tensor.dim")),
            ("tensor", "rows") => Some(host_int("spectra.std.tensor.rows")),
            ("tensor", "cols") => Some(host_int("spectra.std.tensor.cols")),
            ("tensor", "is_valid") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.tensor.is_valid",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("tensor", "get") => Some(host_int("spectra.std.tensor.get")),
            ("tensor", "get_f") => Some(host_float("spectra.std.tensor.get_f")),
            ("tensor", "set") => Some(host_void("spectra.std.tensor.set")),
            ("tensor", "set_f") => Some(host_void("spectra.std.tensor.set_f")),
            ("tensor", "get2") => Some(host_int("spectra.std.tensor.get2")),
            ("tensor", "get2_f") => Some(host_float("spectra.std.tensor.get2_f")),
            ("tensor", "set2") => Some(host_void("spectra.std.tensor.set2")),
            ("tensor", "set2_f") => Some(host_void("spectra.std.tensor.set2_f")),
            ("tensor", "reshape") => Some(host_int("spectra.std.tensor.reshape")),
            ("tensor", "flatten") => Some(host_int("spectra.std.tensor.flatten")),
            ("tensor", "permute") => Some(host_int("spectra.std.tensor.permute")),
            ("tensor", "slice") => Some(host_int("spectra.std.tensor.slice")),
            ("tensor", "concat") => Some(host_int("spectra.std.tensor.concat")),
            ("tensor", "stack") => Some(host_int("spectra.std.tensor.stack")),
            ("tensor", "add") => Some(host_int("spectra.std.tensor.add")),
            ("tensor", "sub") => Some(host_int("spectra.std.tensor.sub")),
            ("tensor", "mul") => Some(host_tensor_dynamic("spectra.std.tensor.mul")),
            ("tensor", "div") => Some(host_int("spectra.std.tensor.div")),
            ("tensor", "sum") => Some(host_int("spectra.std.tensor.sum")),
            ("tensor", "sum_f") => Some(host_float("spectra.std.tensor.sum_f")),
            ("tensor", "sum_t") => Some(host_tensor_rank0("spectra.std.tensor.sum_t")),
            ("tensor", "mean_f") => Some(host_float("spectra.std.tensor.mean_f")),
            ("tensor", "mean_t") => Some(host_int("spectra.std.tensor.mean_t")),
            ("tensor", "max") => Some(host_int("spectra.std.tensor.max")),
            ("tensor", "min") => Some(host_int("spectra.std.tensor.min")),
            ("tensor", "argmax") => Some(host_int("spectra.std.tensor.argmax")),
            ("tensor", "matmul") => Some(host_int("spectra.std.tensor.matmul")),
            ("tensor", "matmul_batched") => Some(host_int("spectra.std.tensor.matmul_batched")),
            ("tensor", "transpose") => Some(host_int("spectra.std.tensor.transpose")),
            ("tensor", "dot") => Some(host_int("spectra.std.tensor.dot")),
            ("tensor", "dot_t") => Some(host_int("spectra.std.tensor.dot_t")),
            ("tensor", "neg") => Some(host_int("spectra.std.tensor.neg")),
            ("tensor", "exp_f") => Some(host_int("spectra.std.tensor.exp_f")),
            ("tensor", "log_f") => Some(host_int("spectra.std.tensor.log_f")),
            ("tensor", "sqrt_f") => Some(host_int("spectra.std.tensor.sqrt_f")),
            ("tensor", "relu") => Some(host_int("spectra.std.tensor.relu")),
            ("tensor", "sigmoid_f") => Some(host_int("spectra.std.tensor.sigmoid_f")),
            ("tensor", "tanh_f") => Some(host_int("spectra.std.tensor.tanh_f")),
            ("tensor", "seed") => Some(host_void("spectra.std.tensor.seed")),
            ("tensor", "uniform") => Some(host_int("spectra.std.tensor.uniform")),
            ("tensor", "uniform_f") => Some(host_int("spectra.std.tensor.uniform_f")),
            ("tensor", "normal_f") => Some(host_int("spectra.std.tensor.normal_f")),
            ("tensor", "bernoulli") => Some(host_int("spectra.std.tensor.bernoulli")),
            ("tensor", "categorical") => Some(host_int("spectra.std.tensor.categorical")),
            ("tensor", "set_deterministic_mode") => {
                Some(host_int("spectra.std.tensor.set_deterministic_mode"))
            }
            ("tensor", "deterministic_mode") => {
                Some(host_int("spectra.std.tensor.deterministic_mode"))
            }
            ("tensor", "tolerance_abs") => Some(host_float("spectra.std.tensor.tolerance_abs")),
            ("tensor", "tolerance_rel") => Some(host_float("spectra.std.tensor.tolerance_rel")),
            ("tensor", "device") => Some(host_int("spectra.std.tensor.device")),
            ("tensor", "device_available") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.tensor.device_available",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("tensor", "device_status") => Some(host_int("spectra.std.tensor.device_status")),
            ("tensor", "to_device") => Some(host_int("spectra.std.tensor.to_device")),
            ("tensor", "cpu") => Some(host_int("spectra.std.tensor.cpu")),
            ("tensor", "sync") => Some(host_void("spectra.std.tensor.sync")),
            ("tensor", "precision") => Some(host_int("spectra.std.tensor.precision")),
            ("tensor", "to_precision") => Some(host_int("spectra.std.tensor.to_precision")),
            ("tensor", "stats_allocations") => {
                Some(host_int("spectra.std.tensor.stats_allocations"))
            }
            ("tensor", "stats_active") => Some(host_int("spectra.std.tensor.stats_active")),
            ("tensor", "stats_peak_bytes") => Some(host_int("spectra.std.tensor.stats_peak_bytes")),
            ("tensor", "stats_reused_buffers") => {
                Some(host_int("spectra.std.tensor.stats_reused_buffers"))
            }
            ("tensor", "stats_pool_hits") => Some(host_int("spectra.std.tensor.stats_pool_hits")),
            ("tensor", "stats_pool_misses") => {
                Some(host_int("spectra.std.tensor.stats_pool_misses"))
            }
            ("tensor", "stats_active_bytes") => {
                Some(host_int("spectra.std.tensor.stats_active_bytes"))
            }
            ("tensor", "stats_scratch_reuses") => {
                Some(host_int("spectra.std.tensor.stats_scratch_reuses"))
            }
            ("tensor", "kernel_strategy") => Some(host_int("spectra.std.tensor.kernel_strategy")),
            ("tensor", "stats_kernel_ops") => Some(host_int("spectra.std.tensor.stats_kernel_ops")),
            ("tensor", "stats_kernel_elements") => {
                Some(host_int("spectra.std.tensor.stats_kernel_elements"))
            }
            ("tensor", "stats_device_transfers") => {
                Some(host_int("spectra.std.tensor.stats_device_transfers"))
            }
            ("tensor", "stats_gpu_kernel_ops") => {
                Some(host_int("spectra.std.tensor.stats_gpu_kernel_ops"))
            }
            ("tensor", "stats_cpu_fallbacks") => {
                Some(host_int("spectra.std.tensor.stats_cpu_fallbacks"))
            }
            ("tensor", "stats_graph_nodes") => {
                Some(host_int("spectra.std.tensor.stats_graph_nodes"))
            }
            ("tensor", "stats_lifetime_records") => {
                Some(host_int("spectra.std.tensor.stats_lifetime_records"))
            }
            ("tensor", "stats_released_lifetimes") => {
                Some(host_int("spectra.std.tensor.stats_released_lifetimes"))
            }
            ("tensor", "stats_allocation_sites") => {
                Some(host_int("spectra.std.tensor.stats_allocation_sites"))
            }
            ("tensor", "stats_reuse_rate_per_mille") => {
                Some(host_int("spectra.std.tensor.stats_reuse_rate_per_mille"))
            }
            ("tensor", "memory_report") => Some(host_string("spectra.std.tensor.memory_report")),
            ("tensor", "reset_stats") => Some(host_void("spectra.std.tensor.reset_stats")),
            ("tensor", "requires_grad") => {
                Some(host_tensor_dynamic("spectra.std.tensor.requires_grad"))
            }
            ("tensor", "diff") => Some(host_void("spectra.std.tensor.backward")),
            ("tensor", "backward") => Some(host_void("spectra.std.tensor.backward")),
            ("tensor", "grad") => Some(host_tensor_dynamic("spectra.std.tensor.grad")),
            ("tensor", "zero_grad") => Some(host_void("spectra.std.tensor.zero_grad")),
            ("tensor", "set_grad_enabled") => {
                Some(host_void("spectra.std.tensor.set_grad_enabled"))
            }
            ("tensor", "grad_enabled") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.tensor.grad_enabled",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("tensor", "free") => Some(host_void("spectra.std.tensor.free")),
            ("tensor", "free_all") => Some(host_int("spectra.std.tensor.free_all")),
            // ── std.ml ───────────────────────────────────────────────────
            ("ml", "module_new") => Some(host_int("spectra.std.ml.module_new")),
            ("ml", "module_add_parameter") => {
                Some(host_void("spectra.std.ml.module_add_parameter"))
            }
            ("ml", "module_parameter_count") => {
                Some(host_int("spectra.std.ml.module_parameter_count"))
            }
            ("ml", "module_parameter") => Some(host_int("spectra.std.ml.module_parameter")),
            ("ml", "module_set_training") => Some(host_void("spectra.std.ml.module_set_training")),
            ("ml", "module_is_training") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.ml.module_is_training",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("ml", "linear") => Some(host_int("spectra.std.ml.linear")),
            ("ml", "conv2d") => Some(host_int("spectra.std.ml.conv2d")),
            ("ml", "dropout") => Some(host_int("spectra.std.ml.dropout")),
            ("ml", "max_pool2d") => Some(host_int("spectra.std.ml.max_pool2d")),
            ("ml", "mse_loss") => Some(host_int("spectra.std.ml.mse_loss")),
            ("ml", "bce_loss") => Some(host_int("spectra.std.ml.bce_loss")),
            ("ml", "cross_entropy_loss") => Some(host_int("spectra.std.ml.cross_entropy_loss")),
            ("ml", "nll_loss") => Some(host_int("spectra.std.ml.nll_loss")),
            ("ml", "sgd_step") => Some(host_void("spectra.std.ml.sgd_step")),
            ("ml", "sgd_momentum_step") => Some(host_void("spectra.std.ml.sgd_momentum_step")),
            ("ml", "adam_step") => Some(host_void("spectra.std.ml.adam_step")),
            ("ml", "adamw_step") => Some(host_void("spectra.std.ml.adamw_step")),
            ("ml", "exp_lr") => Some(host_float("spectra.std.ml.exp_lr")),
            ("ml", "unscale_grad") => Some(host_void("spectra.std.ml.unscale_grad")),
            ("ml", "dataset_from_tensors") => Some(host_int("spectra.std.ml.dataset_from_tensors")),
            ("ml", "dataset_from_csv") => Some(host_int("spectra.std.ml.dataset_from_csv")),
            ("ml", "dataset_from_jsonl") => Some(host_int("spectra.std.ml.dataset_from_jsonl")),
            ("ml", "dataset_from_npy") => Some(host_int("spectra.std.ml.dataset_from_npy")),
            ("ml", "dataset_from_directory") => {
                Some(host_int("spectra.std.ml.dataset_from_directory"))
            }
            ("ml", "dataset_len") => Some(host_int("spectra.std.ml.dataset_len")),
            ("ml", "dataset_map_features") => Some(host_int("spectra.std.ml.dataset_map_features")),
            ("ml", "dataset_filter_label_min") => {
                Some(host_int("spectra.std.ml.dataset_filter_label_min"))
            }
            ("ml", "dataset_train_split") => Some(host_int("spectra.std.ml.dataset_train_split")),
            ("ml", "dataset_test_split") => Some(host_int("spectra.std.ml.dataset_test_split")),
            ("ml", "dataloader_new") => Some(host_int("spectra.std.ml.dataloader_new")),
            ("ml", "dataloader_batch_count") => {
                Some(host_int("spectra.std.ml.dataloader_batch_count"))
            }
            ("ml", "dataloader_batch_features") => {
                Some(host_int("spectra.std.ml.dataloader_batch_features"))
            }
            ("ml", "dataloader_batch_labels") => {
                Some(host_int("spectra.std.ml.dataloader_batch_labels"))
            }
            ("ml", "dataframe_from_csv") => Some(host_int("spectra.std.ml.dataframe_from_csv")),
            ("ml", "dataframe_rows") => Some(host_int("spectra.std.ml.dataframe_rows")),
            ("ml", "dataframe_cols") => Some(host_int("spectra.std.ml.dataframe_cols")),
            ("ml", "dataframe_column") => Some(host_int("spectra.std.ml.dataframe_column")),
            ("ml", "experiment_start") => Some(host_int("spectra.std.ml.experiment_start")),
            ("ml", "experiment_set_config") => {
                Some(host_void("spectra.std.ml.experiment_set_config"))
            }
            ("ml", "experiment_log_metric") => {
                Some(host_void("spectra.std.ml.experiment_log_metric"))
            }
            ("ml", "experiment_log_artifact") => {
                Some(host_void("spectra.std.ml.experiment_log_artifact"))
            }
            ("ml", "experiment_set_lockfile") => {
                Some(host_void("spectra.std.ml.experiment_set_lockfile"))
            }
            ("ml", "experiment_set_model_output") => {
                Some(host_void("spectra.std.ml.experiment_set_model_output"))
            }
            ("ml", "experiment_finish") => Some(host_void("spectra.std.ml.experiment_finish")),
            ("ml", "experiment_manifest_path") => {
                Some(host_string("spectra.std.ml.experiment_manifest_path"))
            }
            ("ml", "experiment_repro_command") => {
                Some(host_string("spectra.std.ml.experiment_repro_command"))
            }
            ("ml", "experiment_compare_manifests") => {
                Some(host_int("spectra.std.ml.experiment_compare_manifests"))
            }
            ("ml", "distributed_session_start") => {
                Some(host_int("spectra.std.ml.distributed_session_start"))
            }
            ("ml", "distributed_worker_step") => {
                Some(host_int("spectra.std.ml.distributed_worker_step"))
            }
            ("ml", "distributed_global_step") => {
                Some(host_int("spectra.std.ml.distributed_global_step"))
            }
            ("ml", "distributed_worker_step_count") => {
                Some(host_int("spectra.std.ml.distributed_worker_step_count"))
            }
            ("ml", "distributed_checkpoint_save") => {
                Some(host_string("spectra.std.ml.distributed_checkpoint_save"))
            }
            ("ml", "distributed_resume") => Some(host_int("spectra.std.ml.distributed_resume")),
            ("ml", "distributed_summary") => {
                Some(host_string("spectra.std.ml.distributed_summary"))
            }
            ("ml", "onnx_export") => Some(host_string("spectra.std.ml.onnx_export")),
            ("ml", "onnx_import_summary") => {
                Some(host_string("spectra.std.ml.onnx_import_summary"))
            }
            ("ml", "onnx_validate") => Some(host_int("spectra.std.ml.onnx_validate")),
            ("ml", "onnx_roundtrip") => Some(host_string("spectra.std.ml.onnx_roundtrip")),
            ("ml", "embedding_lookup") => Some(host_int("spectra.std.ml.embedding_lookup")),
            ("ml", "positional_encoding") => Some(host_int("spectra.std.ml.positional_encoding")),
            ("ml", "layer_norm") => Some(host_int("spectra.std.ml.layer_norm")),
            ("ml", "gelu") => Some(host_int("spectra.std.ml.gelu")),
            ("ml", "swiglu") => Some(host_int("spectra.std.ml.swiglu")),
            ("ml", "attention") => Some(host_int("spectra.std.ml.attention")),
            ("ml", "kv_cache_new") => Some(host_int("spectra.std.ml.kv_cache_new")),
            ("ml", "kv_cache_append") => Some(host_int("spectra.std.ml.kv_cache_append")),
            ("ml", "kv_cache_keys") => Some(host_int("spectra.std.ml.kv_cache_keys")),
            ("ml", "kv_cache_values") => Some(host_int("spectra.std.ml.kv_cache_values")),
            ("ml", "kv_cache_len") => Some(host_int("spectra.std.ml.kv_cache_len")),
            ("ml", "logits_sample") => Some(host_int("spectra.std.ml.logits_sample")),
            ("ml", "tokenizer_wordpiece") => Some(host_int("spectra.std.ml.tokenizer_wordpiece")),
            ("ml", "tokenizer_encode") => Some(host_int("spectra.std.ml.tokenizer_encode")),
            ("ml", "tokenizer_decode") => Some(host_string("spectra.std.ml.tokenizer_decode")),
            ("ml", "text_embed") => Some(host_int("spectra.std.ml.text_embed")),
            ("ml", "vector_index_new") => Some(host_int("spectra.std.ml.vector_index_new")),
            ("ml", "vector_index_insert") => Some(host_int("spectra.std.ml.vector_index_insert")),
            ("ml", "vector_index_query") => Some(host_string("spectra.std.ml.vector_index_query")),
            ("ml", "vector_index_persist") => {
                Some(host_string("spectra.std.ml.vector_index_persist"))
            }
            ("ml", "vector_index_load") => Some(host_int("spectra.std.ml.vector_index_load")),
            ("ml", "rag_chunk_text") => Some(host_string("spectra.std.ml.rag_chunk_text")),
            ("ml", "rag_build_prompt") => Some(host_string("spectra.std.ml.rag_build_prompt")),
            ("ml", "rag_evaluate_answer") => Some(host_int("spectra.std.ml.rag_evaluate_answer")),
            ("ml", "metrics_classification") => {
                Some(host_string("spectra.std.ml.metrics_classification"))
            }
            ("ml", "metrics_regression") => Some(host_string("spectra.std.ml.metrics_regression")),
            ("ml", "metrics_ranking") => Some(host_string("spectra.std.ml.metrics_ranking")),
            ("ml", "metrics_generation") => Some(host_string("spectra.std.ml.metrics_generation")),
            ("ml", "serving_metrics") => Some(host_string("spectra.std.ml.serving_metrics")),
            ("ml", "evaluation_report") => Some(host_string("spectra.std.ml.evaluation_report")),
            // ── std.concurrent ───────────────────────────────────────────
            ("concurrent", "task_spawn") => Some(host_int("spectra.std.concurrent.task_spawn")),
            ("concurrent", "task_join") => Some(host_int("spectra.std.concurrent.task_join")),
            ("concurrent", "task_is_done") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.concurrent.task_is_done",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("concurrent", "channel_new") => Some(host_int("spectra.std.concurrent.channel_new")),
            ("concurrent", "channel_send") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.concurrent.channel_send",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("concurrent", "channel_recv") => Some(host_int("spectra.std.concurrent.channel_recv")),
            ("concurrent", "channel_len") => Some(host_int("spectra.std.concurrent.channel_len")),
            ("concurrent", "channel_close") => {
                Some(host_void("spectra.std.concurrent.channel_close"))
            }
            ("concurrent", "counter_new") => Some(host_int("spectra.std.concurrent.counter_new")),
            ("concurrent", "counter_add") => Some(host_int("spectra.std.concurrent.counter_add")),
            ("concurrent", "counter_get") => Some(host_int("spectra.std.concurrent.counter_get")),
            ("concurrent", "pipeline_sum") => Some(host_int("spectra.std.concurrent.pipeline_sum")),
            ("concurrent", "stats_tasks_spawned") => {
                Some(host_int("spectra.std.concurrent.stats_tasks_spawned"))
            }
            ("concurrent", "stats_channels") => {
                Some(host_int("spectra.std.concurrent.stats_channels"))
            }
            ("concurrent", "reset") => Some(host_void("spectra.std.concurrent.reset")),
            // ── std.serve ────────────────────────────────────────────────
            ("serve", "server_new") => Some(host_int("spectra.std.serve.server_new")),
            ("serve", "server_warmup") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_warmup",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_is_warm") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_is_warm",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_enqueue") => Some(host_int("spectra.std.serve.server_enqueue")),
            ("serve", "server_cancel") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_cancel",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_process_batch") => {
                Some(host_int("spectra.std.serve.server_process_batch"))
            }
            ("serve", "server_result") => Some(host_int("spectra.std.serve.server_result")),
            ("serve", "server_pending") => Some(host_int("spectra.std.serve.server_pending")),
            ("serve", "server_set_timeout") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_set_timeout",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_resident_model") => {
                Some(host_int("spectra.std.serve.server_resident_model"))
            }
            ("serve", "server_benchmark") => Some(host_int("spectra.std.serve.server_benchmark")),
            ("serve", "server_set_input_policy") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_set_input_policy",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_set_output_policy") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_set_output_policy",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_set_rate_limit") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_set_rate_limit",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_set_fallback") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.serve.server_set_fallback",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("serve", "server_last_diagnostic") => {
                Some(host_string("spectra.std.serve.server_last_diagnostic"))
            }
            ("serve", "server_audit_log") => {
                Some(host_string("spectra.std.serve.server_audit_log"))
            }
            ("serve", "reset") => Some(host_void("spectra.std.serve.reset")),
            // ── std.collections map ──────────────────────────────────────
            ("collections", "map_new") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_new",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "map_set") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_set",
                return_type: IRType::Int,
                returns_value: false,
            }),
            ("collections", "map_get") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_get",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "map_contains") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_contains",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "map_remove") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_remove",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "map_len") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_len",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "map_clear") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_clear",
                return_type: IRType::Int,
                returns_value: false,
            }),
            ("collections", "map_free") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.map_free",
                return_type: IRType::Void,
                returns_value: false,
            }),
            // ── std.string ────────────────────────────────────────────────
            ("string", "len") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.len",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("string", "contains") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.contains",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("string", "to_upper") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.to_upper",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "to_lower") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.to_lower",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "trim") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.trim",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "starts_with") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.starts_with",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("string", "ends_with") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.ends_with",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("string", "concat") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.concat",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "repeat_str") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.repeat_str",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "char_at") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.char_at",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("string", "substring") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.substring",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "replace") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.replace",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "index_of") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.index_of",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("string", "split_first") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.split_first",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "split_last") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.split_last",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "is_empty") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.is_empty",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("string", "count_occurrences") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.count_occurrences",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("string", "reverse_str") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.reverse_str",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "pad_left") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.pad_left",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "pad_right") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.pad_right",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("string", "split_by") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.string.split_by",
                return_type: IRType::Int,
                returns_value: true,
            }),
            // ── std.convert ───────────────────────────────────────────────
            ("convert", "int_to_string") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.int_to_string",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("convert", "float_to_string") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.float_to_string",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("convert", "bool_to_string") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.bool_to_string",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("convert", "string_to_int") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.string_to_int",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("convert", "string_to_float") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.string_to_float",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("convert", "int_to_float") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.int_to_float",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("convert", "float_to_int") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.float_to_int",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("convert", "string_to_int_or") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.string_to_int_or",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("convert", "string_to_float_or") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.string_to_float_or",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("convert", "string_to_bool") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.string_to_bool",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("convert", "bool_to_int") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.convert.bool_to_int",
                return_type: IRType::Int,
                returns_value: true,
            }),
            // ── std.char ──────────────────────────────────────────────────
            ("char", "is_alpha") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.is_alpha",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("char", "is_digit_char") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.is_digit_char",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("char", "is_whitespace_char") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.is_whitespace_char",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("char", "is_upper_char") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.is_upper_char",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("char", "is_lower_char") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.is_lower_char",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            ("char", "to_upper_char") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.to_upper_char",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("char", "to_lower_char") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.to_lower_char",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("char", "is_alphanumeric") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.char.is_alphanumeric",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            // ── std.time ──────────────────────────────────────────────────
            ("time", "time_now_millis") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.time.time_now_millis",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("time", "time_now_secs") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.time.time_now_secs",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("time", "sleep_ms") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.time.sleep_ms",
                return_type: IRType::Void,
                returns_value: false,
            }),
            // ── std.random ────────────────────────────────────────────────
            ("random", "random_seed") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.random.random_seed",
                return_type: IRType::Void,
                returns_value: false,
            }),
            ("random", "random_int") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.random.random_int",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("random", "random_float") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.random.random_float",
                return_type: IRType::Float,
                returns_value: true,
            }),
            ("random", "random_bool") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.random.random_bool",
                return_type: IRType::Bool,
                returns_value: true,
            }),
            // ── std.collections extras ────────────────────────────────────
            ("collections", "list_pop") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_pop",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_pop_front") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_pop_front",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_insert_at") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_insert_at",
                return_type: IRType::Void,
                returns_value: false,
            }),
            ("collections", "list_remove_at") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_remove_at",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_index_of") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_index_of",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_sort") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_sort",
                return_type: IRType::Void,
                returns_value: false,
            }),
            ("collections", "list_map") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_map",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_filter") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_filter",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_reduce") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_reduce",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("collections", "list_sort_by") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.collections.list_sort_by",
                return_type: IRType::Void,
                returns_value: false,
            }),
            // ── std.fs ────────────────────────────────────────────────────
            ("fs", "fs_read") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.fs.fs_read",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("fs", "fs_write") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.fs.fs_write",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("fs", "fs_append") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.fs.fs_append",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("fs", "fs_exists") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.fs.fs_exists",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("fs", "fs_remove") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.fs.fs_remove",
                return_type: IRType::Int,
                returns_value: true,
            }),
            // ── std.env ───────────────────────────────────────────────────
            ("env", "env_get") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.env.env_get",
                return_type: IRType::String,
                returns_value: true,
            }),
            ("env", "env_set") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.env.env_set",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("env", "env_args_count") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.env.env_args_count",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("env", "env_arg") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.env.env_arg",
                return_type: IRType::String,
                returns_value: true,
            }),
            // ── std.option ────────────────────────────────────────────────
            ("option", "is_some") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.option.is_some",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("option", "is_none") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.option.is_none",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("option", "option_unwrap") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.option.option_unwrap",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("option", "option_unwrap_or") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.option.option_unwrap_or",
                return_type: IRType::Int,
                returns_value: true,
            }),
            // ── std.result ────────────────────────────────────────────────
            ("result", "is_ok") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.result.is_ok",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("result", "is_err") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.result.is_err",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("result", "result_unwrap") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.result.result_unwrap",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("result", "result_unwrap_or") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.result.result_unwrap_or",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("result", "result_unwrap_err") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.result.result_unwrap_err",
                return_type: IRType::Int,
                returns_value: true,
            }),
            _ => None,
        },
        _ => None,
    }
}

fn host_int(runtime_name: &'static str) -> HostFunctionDescriptor {
    HostFunctionDescriptor {
        runtime_name,
        return_type: IRType::Int,
        returns_value: true,
    }
}

fn host_float(runtime_name: &'static str) -> HostFunctionDescriptor {
    HostFunctionDescriptor {
        runtime_name,
        return_type: IRType::Float,
        returns_value: true,
    }
}

fn host_string(runtime_name: &'static str) -> HostFunctionDescriptor {
    HostFunctionDescriptor {
        runtime_name,
        return_type: IRType::String,
        returns_value: true,
    }
}

fn host_void(runtime_name: &'static str) -> HostFunctionDescriptor {
    HostFunctionDescriptor {
        runtime_name,
        return_type: IRType::Void,
        returns_value: false,
    }
}

fn host_tensor_rank0(runtime_name: &'static str) -> HostFunctionDescriptor {
    HostFunctionDescriptor {
        runtime_name,
        return_type: IRType::Tensor {
            dtype: Box::new(IRType::Float),
            rank: Some(0),
            dims: None,
            layout: None,
            device: None,
        },
        returns_value: true,
    }
}

fn host_tensor_dynamic(runtime_name: &'static str) -> HostFunctionDescriptor {
    HostFunctionDescriptor {
        runtime_name,
        return_type: IRType::Tensor {
            dtype: Box::new(IRType::Float),
            rank: None,
            dims: None,
            layout: None,
            device: None,
        },
        returns_value: true,
    }
}

#[derive(Debug, Default)]
struct LoweredTensorMetadata {
    rank: Option<usize>,
    dims: Option<Vec<Option<usize>>>,
    layout: Option<String>,
    device: Option<String>,
}

fn tensor_metadata(type_args: &[TypeAnnotation]) -> LoweredTensorMetadata {
    let mut meta = LoweredTensorMetadata::default();
    let mut dims = Vec::new();

    for ann in type_args {
        let TypeAnnotationKind::Simple { segments } = &ann.kind else {
            continue;
        };
        if segments.len() != 1 {
            continue;
        }
        let name = segments[0].clone();

        if let Some(rank) = name
            .strip_prefix("rank")
            .and_then(|raw| raw.parse::<usize>().ok())
        {
            meta.rank = Some(rank);
            continue;
        }
        if let Some(dim) = name
            .strip_prefix("dim")
            .and_then(|raw| raw.parse::<usize>().ok())
        {
            dims.push(Some(dim));
            continue;
        }
        match name.as_str() {
            "dyn" | "dynamic_dim" | "dim_dynamic" => dims.push(None),
            "dynamic" => meta.rank = None,
            "row_major" | "col_major" | "contiguous" | "strided" => meta.layout = Some(name),
            "cpu" | "wgpu" | "cuda" | "rocm" | "metal" | "directml" | "vulkan" => {
                meta.device = Some(name)
            }
            _ => {}
        }
    }

    if !dims.is_empty() {
        if meta.rank.is_none() {
            meta.rank = Some(dims.len());
        }
        meta.dims = Some(dims);
    }

    meta
}

impl Default for ASTLowering {
    fn default() -> Self {
        Self::new()
    }
}
