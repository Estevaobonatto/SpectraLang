// AST to IR lowering pass
// Converts semantic AST to SSA-based IR

use crate::builder::IRBuilder;
use crate::ir::{
    Function as IRFunction, Module as IRModule, Parameter, Terminator, Type as IRType, Value,
};
use spectra_compiler::ast::{
    BinaryOperator, Block, Enum as ASTEnum, Expression, ExpressionKind, FStringPart,
    Function as ASTFunction, IfLetStatement, Item, Module as ASTModule, Statement, StatementKind,
    Struct as ASTStruct, Type as ASTType, TypeAnnotation, TypeAnnotationKind, UnaryOperator,
    WhileLetStatement,
};
use spectra_compiler::span::Span;
use std::collections::HashMap;

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
}

impl ASTLowering {
    pub fn new() -> Self {
        Self {
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
        }
    }

    pub fn lower_module(&mut self, ast_module: &ASTModule) -> IRModule {
        let mut ir_module = IRModule::new(&ast_module.name);

        // First pass: collect struct and enum definitions, and trait implementations
        for item in &ast_module.items {
            if let Item::Struct(struct_def) = item {
                // Check if this is a generic struct
                if !struct_def.type_params.is_empty() {
                    // Store generic struct for later monomorphization
                    self.generic_structs
                        .insert(struct_def.name.clone(), struct_def.clone());
                    eprintln!(
                        "Info: Stored generic struct '{}' for monomorphization",
                        struct_def.name
                    );
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
                    // Store generic enum for later monomorphization
                    self.generic_enums
                        .insert(enum_def.name.clone(), enum_def.clone());
                    eprintln!(
                        "Info: Stored generic enum '{}' for monomorphization",
                        enum_def.name
                    );
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
                // Collect trait implementations
                if let Some(ref trait_name) = impl_block.trait_name {
                    let key = (impl_block.type_name.clone(), trait_name.clone());
                    self.trait_implementations.insert(key, true);
                }
            }
        }

        // Second pass: lower functions
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
            }
        }

        for item in &ast_module.items {
            if let Item::Function(func) = item {
                // Store generic functions for later monomorphization
                if !func.type_params.is_empty() {
                    self.generic_functions
                        .insert(func.name.clone(), func.clone());
                    eprintln!(
                        "Info: Stored generic function '{}' for monomorphization",
                        func.name
                    );
                    continue;
                }

                let ir_func = self.lower_function(func);
                ir_module.add_function(ir_func);
            }
        }

        // Process pending monomorphization requests
        self.process_monomorphization_requests(&mut ir_module);

        ir_module
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
                    "Warning: monomorphization limit ({}) reached — possible infinite \
                     expansion involving '{}'. Remaining specializations skipped.",
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
                eprintln!("Info: Generating specialization: {}", mangled);

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
                    "Warning: Generic function '{}' not found for monomorphization",
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
                        panic!(
                            "Trait bound violation: Type '{}' does not implement trait '{}' required by type parameter '{}'.\n\
                             Function '{}' requires {} to have trait {}.\n\
                             Specialization: {} -> {}",
                            type_name, bound, type_param.name,
                            generic_func.name, type_param.name, bound,
                            generic_func.name, request.mangled_name()
                        );
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
            TypeAnnotationKind::Function { params, return_type } => {
                for param in params {
                    self.substitute_type_in_annotation(param, type_map);
                }
                self.substitute_type_in_annotation(return_type, type_map);
            }
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

            panic!(
                "Struct '{}' não foi registrada antes do lowering; verifique a etapa semântica",
                base_name
            );
        }

        let type_names: Vec<String> = type_args
            .iter()
            .map(|ty| self.type_annotation_to_string(ty))
            .collect();
        let mangled = format!("{}_{}", base_name, type_names.join("_"));

        if !self.struct_definitions.contains_key(&mangled) {
            let generic_struct =
                self.generic_structs
                    .get(base_name)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                    "Struct genérica '{}' não encontrada para especialização com argumentos {:?}",
                    base_name, type_names
                )
                    });

            self.specialize_struct(&generic_struct, type_args, &mangled);
        }

        let fields = self
            .struct_definitions
            .get(&mangled)
            .cloned()
            .unwrap_or_else(|| panic!("Struct '{}' não registrada após especialização", mangled));

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

            panic!(
                "Enum '{}' não foi registrado antes do lowering; verifique a etapa semântica",
                base_name
            );
        }

        let type_names: Vec<String> = type_args
            .iter()
            .map(|ty| self.type_annotation_to_string(ty))
            .collect();
        let mangled = format!("{}_{}", base_name, type_names.join("_"));

        if !self.enum_definitions.contains_key(&mangled) {
            let generic_enum = self
                .generic_enums
                .get(base_name)
                .cloned()
                .unwrap_or_else(|| {
                    panic!(
                        "Enum genérico '{}' não encontrado para especialização com argumentos {:?}",
                        base_name, type_names
                    )
                });

            self.specialize_enum(&generic_enum, type_args, &mangled);
        }

        let variants = self
            .enum_definitions
            .get(&mangled)
            .cloned()
            .unwrap_or_else(|| panic!("Enum '{}' não registrado após especialização", mangled));

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
            IRType::Struct { name, .. } => Self::simple_type_annotation(name),
            IRType::Enum { name, .. } => Self::simple_type_annotation(name),
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
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
            } => {
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
            ExpressionKind::Lambda { .. } => IRType::Int, // TODO: function pointer type
            ExpressionKind::Try(inner) => self.infer_expr_ir_type(inner),
            ExpressionKind::Range { .. } => IRType::Array {
                element_type: Box::new(IRType::Int),
                size: 0,
            },
            ExpressionKind::Block(block) => {
                block.statements.last()
                    .and_then(|stmt| {
                        if let spectra_compiler::ast::StatementKind::Expression(expr) = &stmt.kind {
                            Some(self.infer_expr_ir_type(expr))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(IRType::Void)
            }
        }
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

        let mut ir_func = IRFunction::new(&ast_func.name, params.clone(), return_type);

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
                // Lower last expression and capture its value
                implicit_return_value = Some(self.lower_expression(expr, &mut ir_func));
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
            self.struct_var_map.pop_scope();
            self.array_map.pop_scope();
            self.variable_types.pop_scope();
            self.value_map.pop_scope();
        }
    }

    /// Analyzes which variables are assigned to in a block
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
        self.lower_block(&block.statements, ir_func);

        let current_block_id = self.builder.get_current_block().unwrap_or(entry_block);

        let produced_value = block.statements.last().and_then(|stmt| match &stmt.kind {
            StatementKind::Expression(expr) => Some(self.lower_expression(expr, ir_func)),
            _ => None,
        });

        let has_terminator = ir_func
            .get_block(current_block_id)
            .map(|block| block.terminator.is_some())
            .unwrap_or(false);

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

    fn lower_statement(&mut self, stmt: &Statement, ir_func: &mut IRFunction) {
        match &stmt.kind {
            StatementKind::Let(let_stmt) => {
                // Discover variable type either from initializer or annotation
                let inferred_type = if let Some(ref value_expr) = let_stmt.value {
                    Some(self.infer_expr_ir_type(value_expr))
                } else if let Some(ref type_ann) = let_stmt.ty {
                    Some(self.lower_type_annotation(type_ann))
                } else {
                    None
                };

                if let Some(ref ty) = inferred_type {
                    self.variable_types
                        .insert(let_stmt.name.clone(), ty.clone());
                }

                if let Some(ref value_expr) = let_stmt.value {
                    let value = self.lower_expression(value_expr, ir_func);

                    match &value_expr.kind {
                        ExpressionKind::ArrayLiteral { .. } => {
                            if let Some(IRType::Array { element_type, size }) =
                                inferred_type.clone()
                            {
                                self.array_map.insert(
                                    let_stmt.name.clone(),
                                    ArrayInfo {
                                        ptr: value,
                                        element_type: *element_type,
                                        size,
                                    },
                                );
                            }
                            self.value_map.insert(let_stmt.name.clone(), value);
                        }
                        ExpressionKind::StructLiteral {
                            name, type_args, ..
                        } => {
                            let (actual_name, _) =
                                self.ensure_struct_definition(name, type_args.as_slice());
                            self.struct_var_map
                                .insert(let_stmt.name.clone(), (value, actual_name.clone()));
                            self.value_map.insert(let_stmt.name.clone(), value);
                        }
                        _ => {
                            if let Some(ref type_ann) = let_stmt.ty {
                                let var_type = self.lower_type_annotation(type_ann);
                                if let IRType::Struct { name, fields } = var_type {
                                    let struct_ptr = self.builder.build_alloca(
                                        ir_func,
                                        IRType::Struct {
                                            name: name.clone(),
                                            fields: fields.clone(),
                                        },
                                    );
                                    self.builder.build_store(ir_func, struct_ptr, value);
                                    self.struct_var_map
                                        .insert(let_stmt.name.clone(), (struct_ptr, name));
                                    self.value_map.insert(let_stmt.name.clone(), struct_ptr);
                                } else if let Some(&alloca_ptr) =
                                    self.alloca_map.get(&let_stmt.name)
                                {
                                    self.builder.build_store(ir_func, alloca_ptr, value);
                                } else {
                                    self.value_map.insert(let_stmt.name.clone(), value);
                                }
                            } else if let Some(&alloca_ptr) = self.alloca_map.get(&let_stmt.name) {
                                self.builder.build_store(ir_func, alloca_ptr, value);
                            } else {
                                self.value_map.insert(let_stmt.name.clone(), value);
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
                // Lower iterable expression once to avoid recomputation
                let iterable_value = self.lower_expression(&for_stmt.iterable, ir_func);
                let iterable_type = self.infer_expr_ir_type(&for_stmt.iterable);

                let (element_type, length) = match iterable_type {
                    IRType::Array { element_type, size } => (*element_type, size),
                    other => panic!(
                        "for-loop lowering currently supports arrays only, found {:?}",
                        other
                    ),
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
                let element_value = self.builder.build_load(ir_func, element_ptr);

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
                            panic!(
                                "Switch case pattern must be a constant integer expression, found {:?}",
                                case.pattern.kind
                            )
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
                value,
                then_block,
                else_block,
                ..
            }) => {
                // TODO: full pattern matching — for now lower as plain block
                let _val = self.lower_expression(value, ir_func);
                self.lower_block(&then_block.statements, ir_func);
                if let Some(else_b) = else_block {
                    self.lower_block(&else_b.statements, ir_func);
                }
            }
            StatementKind::WhileLet(WhileLetStatement { value, body, .. }) => {
                // TODO: full pattern matching — for now lower as plain block
                let _val = self.lower_expression(value, ir_func);
                self.lower_block(&body.statements, ir_func);
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
        lookup_std_host_function(&path)
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
                if let Some(info) = self.array_map.get(name) {
                    info.ptr
                }
                // Check if this is a struct variable
                else if let Some((struct_ptr, _)) = self.struct_var_map.get(name) {
                    // Load struct from memory
                    self.builder.build_load(ir_func, struct_ptr)
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
                let operand_value = self.lower_expression(operand, ir_func);

                match operator {
                    UnaryOperator::Negate => {
                        // Negate: 0 - operand
                        let zero = self.builder.build_const_int(ir_func, 0);
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

                // temporary bypass for closures
                if !self.function_return_types.contains_key(&function_name) 
                   && !self.generic_functions.contains_key(&function_name) 
                   && function_name != "unknown" {
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
                        eprintln!(
                            "Info: Requesting specialization: {} -> {}",
                            function_name, mangled
                        );
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
                    // Both branches have terminators (returns), no merge needed
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

                let elem_ptr =
                    self.builder
                        .build_getelementptr(ir_func, tuple_ptr, index_value, elem_type);

                // Carregar o valor do elemento
                self.builder.build_load(ir_func, elem_ptr)
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
                            panic!(
                                "Campo '{}' não encontrado na definição de struct '{}'",
                                field_name, actual_name
                            );
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
                                return self.builder.build_load(ir_func, field_ptr);
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
                        return self.builder.build_load(ir_func, field_ptr);
                    }
                }

                ir_func.next_value()
            }
            ExpressionKind::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
            } => {
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
                        // Se é unit variant, retornar apenas o tag
                        if variant_data_types.is_none() {
                            return self.builder.build_const_int(ir_func, *tag as i64);
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
                    self.infer_expr_ir_type(&first_arm.body)
                } else {
                    IRType::Int
                };
                for arm in arms.iter().skip(1) {
                    let arm_type = self.infer_expr_ir_type(&arm.body);
                    if let Some(merged) = self.merge_types(&result_type, &arm_type) {
                        result_type = merged;
                    }
                }

                let result_alloca = self.builder.build_alloca(ir_func, result_type.clone());

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

                    let body_value = self.lower_expression(&arm.body, ir_func);
                    self.builder.build_store(ir_func, result_alloca, body_value);
                    self.builder.build_branch(ir_func, exit_block);

                    self.struct_var_map.pop_scope();
                    self.array_map.pop_scope();
                    self.variable_types.pop_scope();
                    self.value_map.pop_scope();
                }

                // Bloco de saída
                self.builder.set_current_block(exit_block);
                self.builder.build_load(ir_func, result_alloca)
            }
            ExpressionKind::MethodCall {
                object,
                method_name,
                arguments,
                type_name,
            } => {
                // Lower method call to function call: obj.method(args) -> Type_method(obj, args)

                // 1. Lower o objeto (self será o primeiro argumento)
                let obj_value = self.lower_expression(object, ir_func);

                // 2. Determinar o tipo do objeto
                let obj_type_name = if let Some(name) = type_name {
                    // Tipo já foi preenchido pelo semantic analyzer
                    name.clone()
                } else {
                    match self.infer_expr_ir_type(object) {
                        IRType::Struct { name, .. } => name,
                        IRType::Enum { name, .. } => name,
                        other => panic!(
                            "Não foi possível determinar o tipo do objeto para chamada de método '{method_name}' (tipo inferido: {:?})",
                            other
                        ),
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
            ExpressionKind::CharLiteral(c) => {
                self.builder.build_const_int(ir_func, *c as i64)
            }
            ExpressionKind::FString(parts) => {
                let lowered: Vec<Value> = parts
                    .iter()
                    .map(|part| match part {
                        FStringPart::Literal(s) => self.lower_string_literal(s, ir_func),
                        FStringPart::Interpolated(expr) => self.lower_expression(expr, ir_func),
                    })
                    .collect();
                if lowered.is_empty() {
                    return self.lower_string_literal("", ir_func);
                }
                let mut result = lowered[0];
                for part in &lowered[1..] {
                    result = self.builder.build_add(ir_func, result, *part);
                }
                result
            }
            ExpressionKind::Try(inner) => {
                // TODO: proper error propagation — lower inner and pass through
                self.lower_expression(inner, ir_func)
            }
            ExpressionKind::Range { start, end, .. } => {
                // TODO: proper range object — lower both bounds and return start for now
                let _end = self.lower_expression(end, ir_func);
                self.lower_expression(start, ir_func)
            }
            ExpressionKind::Lambda { .. } => {
                // TODO: closure lowering
                self.builder.build_const_int(ir_func, 0)
            }
            ExpressionKind::Block(block) => {
                let mut last_val = self.builder.build_const_int(ir_func, 0); // default
                for stmt in &block.statements {
                    if let spectra_compiler::ast::StatementKind::Expression(expr) = &stmt.kind {
                        last_val = self.lower_expression(expr, ir_func);
                    } else {
                        self.lower_statement(stmt, ir_func);
                    }
                }
                last_val
            }
        }
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
                        // Para unit variant, comparar diretamente o tag
                        if variant_types.is_none() {
                            let expected_tag_value =
                                self.builder.build_const_int(ir_func, *expected_tag as i64);
                            return self
                                .builder
                                .build_eq(ir_func, scrutinee, expected_tag_value);
                        } else {
                            // Para tuple variant, extrair tag (primeiro elemento da tuple)
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
                            return self
                                .builder
                                .build_eq(ir_func, tag_value, expected_tag_value);
                        }
                    }
                }
                // Fallback: sempre false
                self.builder.build_const_int(ir_func, 0)
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
            Pattern::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
                struct_data,
                ..
            } => {
                // Se há patterns de data, extrair valores e fazer binding recursivo
                let ordered_patterns: Vec<&spectra_compiler::ast::Pattern> = if let Some(patterns) = data {
                    patterns.iter().collect()
                } else if let Some(named_patterns) = struct_data {
                    self.reorder_named_variant_patterns(scrutinee_enum.unwrap_or(enum_name), variant_name, named_patterns)
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
                                        let element_value =
                                            self.builder.build_load(ir_func, element_ptr);

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
                    "int" => IRType::Int,
                    "float" => IRType::Float,
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
            TypeAnnotationKind::Function { .. } => {
                IRType::Pointer(Box::new(IRType::Void))
            }
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
            ASTType::Fn { .. } => {
                // Closure/function type — represent as a generic pointer for now
                IRType::Pointer(Box::new(IRType::Void))
            }
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
            panic!(
                "Type argument count mismatch for struct '{}': expected {}, got {}",
                generic.name,
                generic.type_params.len(),
                type_args.len()
            );
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

        eprintln!(
            "Info: Specialized struct '{}' as '{}'",
            generic.name, mangled_name
        );
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
            panic!(
                "Type argument count mismatch for enum '{}': expected {}, got {}",
                generic.name,
                generic.type_params.len(),
                type_args.len()
            );
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

        eprintln!(
            "Info: Specialized enum '{}' as '{}'",
            generic.name, mangled_name
        );
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
            TypeAnnotationKind::Function { params, return_type } => {
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
            ("io", "print") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.print",
                return_type: IRType::Int,
                returns_value: true,
            }),
            ("io", "flush") => Some(HostFunctionDescriptor {
                runtime_name: "spectra.std.io.flush",
                return_type: IRType::Int,
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
            _ => None,
        },
        _ => None,
    }
}

impl Default for ASTLowering {
    fn default() -> Self {
        Self::new()
    }
}
