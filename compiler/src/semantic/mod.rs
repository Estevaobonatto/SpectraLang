use crate::{
    ast::{
        Block, Expression, ExpressionKind, Function, Item, Module, Pattern, Statement,
        StatementKind, Type, Visibility,
    },
    error::SemanticError,
    span::Span,
};
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone)]
struct TraitMethodSignature {
    params: Vec<ParameterInfo>,
    return_type: Option<TypeAnnotationPattern>,
    has_default_body: bool,
}

#[derive(Debug, Clone)]
struct ParameterInfo {
    is_self: bool,
    is_reference: bool,
    is_mutable: bool,
    ty: Option<TypeAnnotationPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeAnnotationPattern {
    Simple(Vec<String>),
    Tuple(Vec<TypeAnnotationPattern>),
}

impl fmt::Display for TypeAnnotationPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeAnnotationPattern::Simple(segments) => {
                write!(f, "{}", segments.join("::"))
            }
            TypeAnnotationPattern::Tuple(elements) => {
                write!(f, "(")?;
                for (index, element) in elements.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", element)?;
                }
                write!(f, ")")
            }
        }
    }
}

pub fn analyze_modules(modules: &mut [&mut Module]) -> Result<(), Vec<SemanticError>> {
    let mut errors = Vec::new();

    for module in modules {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer.analyze_module(module);
        errors.extend(analyzer.errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone)]
struct SymbolInfo {
    #[allow(dead_code)]
    span: Span,
    ty: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelfParamKind {
    Value,
    Reference { mutable: bool },
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<Type>,
    return_type: Type,
    self_kind: Option<SelfParamKind>,
}

#[derive(Debug, Clone)]
struct TraitMethodInfo {
    signature: FunctionSignature,
    has_default: bool, // true if method has default implementation
    #[allow(dead_code)]
    default_body: Option<crate::ast::Block>, // corpo da implementação padrão, se houver
}

#[derive(Debug, Clone)]
struct StructFieldInfo {
    ty: crate::ast::TypeAnnotation,
    #[allow(dead_code)]
    span: Span,
}

#[derive(Debug, Clone)]
struct StructInfo {
    visibility: Visibility,
    type_params: Vec<String>,
    fields: HashMap<String, StructFieldInfo>,
}

#[derive(Debug, Clone)]
struct EnumVariantInfo {
    data: Option<Vec<crate::ast::TypeAnnotation>>,
    #[allow(dead_code)]
    span: Span,
}

#[derive(Debug, Clone)]
struct EnumInfo {
    visibility: Visibility,
    type_params: Vec<String>,
    variants: HashMap<String, EnumVariantInfo>,
}

pub struct SemanticAnalyzer {
    errors: Vec<SemanticError>,
    // Symbol table: maps variable/function names to their type info
    symbols: Vec<HashMap<String, SymbolInfo>>,
    // Function table: maps function names to their signatures
    functions: HashMap<String, FunctionSignature>,
    // Enum definitions: maps enum names to their variants
    enum_definitions: HashMap<String, Vec<String>>,
    // Methods: maps type_name to (method_name, signature)
    methods: HashMap<String, HashMap<String, FunctionSignature>>,
    // Traits: maps trait_name to (method_name, method_info)
    traits: HashMap<String, HashMap<String, TraitMethodInfo>>,
    trait_signatures: HashMap<String, HashMap<String, TraitMethodSignature>>,
    // Trait implementations: maps (trait_name, type_name) to validation status
    trait_impls: HashMap<(String, String), bool>,
    // Struct metadata for validation and lookup
    struct_infos: HashMap<String, StructInfo>,
    // Enum metadata (including variant payload types)
    enum_infos: HashMap<String, EnumInfo>,
    // Generic structs: maps struct_name to (type_params, field_definitions)
    generic_structs: HashMap<
        String,
        (
            Vec<crate::ast::TypeParameter>,
            Vec<(String, crate::ast::TypeAnnotation)>,
        ),
    >,
    // Generic enums: maps enum_name to (type_params, variants)
    generic_enums: HashMap<String, (Vec<crate::ast::TypeParameter>, Vec<String>)>,
    // Track if we're inside a loop (for break/continue validation)
    loop_depth: usize,
    // Track if we're inside a function (for return validation)
    current_function: Option<String>,
    current_return_type: Option<Type>,
    // Stack of in-scope generic type parameters
    generic_params: Vec<HashSet<String>>,
    generic_param_bounds: Vec<HashMap<String, Vec<String>>>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            symbols: vec![HashMap::new()], // Start with global scope
            functions: HashMap::new(),
            enum_definitions: HashMap::new(),
            methods: HashMap::new(),
            traits: HashMap::new(),
            trait_impls: HashMap::new(),
            struct_infos: HashMap::new(),
            enum_infos: HashMap::new(),
            generic_structs: HashMap::new(),
            generic_enums: HashMap::new(),
            loop_depth: 0,
            current_function: None,
            trait_signatures: HashMap::new(),
            current_return_type: None,
            generic_params: Vec::new(),
            generic_param_bounds: Vec::new(),
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(SemanticError::new(message, span));
    }

    fn push_scope(&mut self) {
        self.symbols.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.symbols.pop();
    }

    fn push_generic_params(&mut self, params: &[crate::ast::TypeParameter]) -> bool {
        if params.is_empty() {
            return false;
        }
        let mut set = HashSet::new();
        let mut bounds_map = HashMap::new();

        for param in params {
            set.insert(param.name.clone());
            bounds_map.insert(param.name.clone(), param.bounds.clone());

            for bound in &param.bounds {
                if !self.traits.contains_key(bound) {
                    self.error(
                        format!(
                            "Trait '{}' referenced in bound for type parameter '{}' is not defined",
                            bound, param.name
                        ),
                        param.span,
                    );
                }
            }
        }
        self.generic_params.push(set);
        self.generic_param_bounds.push(bounds_map);
        true
    }

    fn pop_generic_params(&mut self) {
        self.generic_params.pop();
        self.generic_param_bounds.pop();
    }

    fn is_generic_param(&self, name: &str) -> bool {
        self.generic_params
            .iter()
            .rev()
            .any(|params| params.contains(name))
    }

    fn get_generic_bounds(&self, name: &str) -> Option<&Vec<String>> {
        for bounds in self.generic_param_bounds.iter().rev() {
            if let Some(list) = bounds.get(name) {
                return Some(list);
            }
        }
        None
    }

    fn trait_method_signature_for_type_param(
        &self,
        param_name: &str,
        method_name: &str,
    ) -> Option<(FunctionSignature, String)> {
        let bounds = self.get_generic_bounds(param_name)?;

        for trait_name in bounds {
            if let Some(trait_methods) = self.traits.get(trait_name) {
                if let Some(trait_method_info) = trait_methods.get(method_name) {
                    let mut params = trait_method_info.signature.params.clone();
                    if trait_method_info.signature.self_kind.is_some() && !params.is_empty() {
                        params[0] = Type::TypeParameter {
                            name: param_name.to_string(),
                        };
                    }

                    let signature = FunctionSignature {
                        params,
                        return_type: trait_method_info.signature.return_type.clone(),
                        self_kind: trait_method_info.signature.self_kind,
                    };

                    return Some((signature, trait_name.clone()));
                }
            }
        }

        None
    }

    fn validate_method_call_signature(
        &mut self,
        method_name: &str,
        signature: &FunctionSignature,
        receiver_type: &Type,
        arguments: &[Expression],
        call_span: Span,
    ) {
        let has_self = signature.self_kind.is_some();

        if !has_self {
            self.error(
                format!(
                    "Method '{}' does not take 'self'; call it as an associated function",
                    method_name
                ),
                call_span,
            );
            return;
        }

        if let Some(expected_self_type) = signature.params.get(0) {
            if !self.types_match(receiver_type, expected_self_type) {
                self.error(
                    format!(
                        "Method '{}' expects receiver of type {:?}, but found {:?}",
                        method_name, expected_self_type, receiver_type
                    ),
                    call_span,
                );
            }
        }

        let arg_offset = if has_self { 1 } else { 0 };
        let expected_args = signature.params.len().saturating_sub(arg_offset);

        if arguments.len() != expected_args {
            self.error(
                format!(
                    "Method '{}' expects {} argument(s), but {} were provided",
                    method_name,
                    expected_args,
                    arguments.len()
                ),
                call_span,
            );
        }

        for (i, arg) in arguments.iter().enumerate() {
            let arg_type = self.infer_expression_type(arg);
            let expected_index = i + arg_offset;
            if let Some(expected_type) = signature.params.get(expected_index) {
                if !self.types_match(&arg_type, expected_type) {
                    let mut message = format!(
                        "Method '{}' argument {} has type {:?}, but {:?} was expected",
                        method_name,
                        i + 1,
                        arg_type,
                        expected_type
                    );
                    if let Some(hint) = self.conversion_hint(&arg_type, expected_type) {
                        message.push_str(" ");
                        message.push_str(&hint);
                    }

                    self.error(message, arg.span);
                }
            }
        }
    }

    fn type_annotation_to_type(&self, type_ann: &Option<crate::ast::TypeAnnotation>) -> Type {
        use crate::ast::TypeAnnotationKind;

        match type_ann {
            Some(ann) => match &ann.kind {
                TypeAnnotationKind::Simple { segments } if segments.len() == 1 => {
                    match segments[0].as_str() {
                        "int" => Type::Int,
                        "float" => Type::Float,
                        "bool" => Type::Bool,
                        "string" => Type::String,
                        "char" => Type::Char,
                        "Self" => Type::SelfType, // Self type
                        other => {
                            if self.is_generic_param(other) {
                                Type::TypeParameter {
                                    name: other.to_string(),
                                }
                            } else {
                                Type::Unknown
                            }
                        }
                    }
                }
                TypeAnnotationKind::Tuple { elements } => {
                    let element_types: Vec<Type> = elements
                        .iter()
                        .map(|elem_ann| self.type_annotation_to_type(&Some(elem_ann.clone())))
                        .collect();
                    Type::Tuple {
                        elements: element_types,
                    }
                }
                _ => Type::Unknown,
            },
            None => Type::Unknown,
        }
    }

    fn annotation_to_pattern(annotation: &crate::ast::TypeAnnotation) -> TypeAnnotationPattern {
        use crate::ast::TypeAnnotationKind;

        match &annotation.kind {
            TypeAnnotationKind::Simple { segments } => {
                TypeAnnotationPattern::Simple(segments.clone())
            }
            TypeAnnotationKind::Tuple { elements } => TypeAnnotationPattern::Tuple(
                elements.iter().map(Self::annotation_to_pattern).collect(),
            ),
        }
    }

    fn option_annotation_to_pattern(
        annotation: &Option<crate::ast::TypeAnnotation>,
    ) -> Option<TypeAnnotationPattern> {
        annotation.as_ref().map(Self::annotation_to_pattern)
    }

    fn format_parameter(param: &ParameterInfo) -> String {
        if param.is_self {
            if param.is_reference {
                if param.is_mutable {
                    "&mut self".to_string()
                } else {
                    "&self".to_string()
                }
            } else {
                "self".to_string()
            }
        } else if let Some(ty) = &param.ty {
            ty.to_string()
        } else {
            "_".to_string()
        }
    }

    fn format_trait_signature(method_name: &str, signature: &TraitMethodSignature) -> String {
        let params = signature
            .params
            .iter()
            .map(Self::format_parameter)
            .collect::<Vec<_>>()
            .join(", ");

        let return_part = signature
            .return_type
            .as_ref()
            .map(|ty| format!(" -> {}", ty))
            .unwrap_or_default();

        format!("fn {}({}){}", method_name, params, return_part)
    }

    fn declare_symbol(&mut self, name: String, span: Span, ty: Type) -> bool {
        // Check if already declared in current scope
        if let Some(current_scope) = self.symbols.last_mut() {
            if current_scope.contains_key(&name) {
                return false; // Already declared
            }
            current_scope.insert(name, SymbolInfo { span, ty });
            true
        } else {
            false
        }
    }

    fn lookup_symbol(&self, name: &str) -> Option<&SymbolInfo> {
        // Search from innermost to outermost scope
        for scope in self.symbols.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    fn is_builtin_type(name: &str) -> bool {
        matches!(name, "int" | "float" | "bool" | "string" | "char" | "Self")
    }

    fn can_auto_promote(&self, from: &Type, to: &Type) -> bool {
        matches!((from, to), (Type::Int, Type::Float))
    }

    fn is_numeric_type(ty: &Type) -> bool {
        matches!(ty, Type::Int | Type::Float)
    }

    fn numeric_types_can_interact(&self, left: &Type, right: &Type) -> bool {
        matches!(left, Type::Unknown)
            || matches!(right, Type::Unknown)
            || (Self::is_numeric_type(left) && Self::is_numeric_type(right))
    }

    fn numeric_result_type(&self, left: &Type, right: &Type) -> Type {
        if matches!(left, Type::Unknown) || matches!(right, Type::Unknown) {
            return Type::Unknown;
        }

        if matches!(left, Type::Float) || matches!(right, Type::Float) {
            Type::Float
        } else if matches!(left, Type::Int) && matches!(right, Type::Int) {
            Type::Int
        } else {
            Type::Unknown
        }
    }

    fn lookup_type_visibility(&self, name: &str) -> Option<Visibility> {
        if let Some(info) = self.struct_infos.get(name) {
            return Some(info.visibility);
        }
        if let Some(info) = self.enum_infos.get(name) {
            return Some(info.visibility);
        }
        None
    }

    fn validate_public_type_annotation(
        &mut self,
        annotation: &crate::ast::TypeAnnotation,
        generics: &HashSet<String>,
        context: &str,
        span: Span,
    ) {
        use crate::ast::TypeAnnotationKind;

        match &annotation.kind {
            TypeAnnotationKind::Simple { segments } => {
                if let Some(name) = segments.last() {
                    if generics.contains(name) || Self::is_builtin_type(name) {
                        return;
                    }

                    if let Some(visibility) = self.lookup_type_visibility(name) {
                        if visibility == Visibility::Private {
                            self.error(
                                format!(
                                    "{} references private type '{}'; mark the referenced type as public or keep this item private",
                                    context, name
                                ),
                                span,
                            );
                        }
                    }
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                for element in elements {
                    self.validate_public_type_annotation(element, generics, context, span);
                }
            }
        }
    }

    fn enforce_visibility_rules(&mut self, item: &Item) {
        match item {
            Item::Struct(struct_def) if struct_def.visibility == Visibility::Public => {
                let generics: HashSet<String> = struct_def
                    .type_params
                    .iter()
                    .map(|tp| tp.name.clone())
                    .collect();

                for field in &struct_def.fields {
                    self.validate_public_type_annotation(
                        &field.ty,
                        &generics,
                        &format!("Public struct '{}' field '{}'", struct_def.name, field.name),
                        field.span,
                    );
                }
            }
            Item::Enum(enum_def) if enum_def.visibility == Visibility::Public => {
                let generics: HashSet<String> = enum_def
                    .type_params
                    .iter()
                    .map(|tp| tp.name.clone())
                    .collect();

                for variant in &enum_def.variants {
                    if let Some(data) = &variant.data {
                        for (index, annotation) in data.iter().enumerate() {
                            self.validate_public_type_annotation(
                                annotation,
                                &generics,
                                &format!(
                                    "Public enum '{}' variant '{}' field {}",
                                    enum_def.name,
                                    variant.name,
                                    index + 1
                                ),
                                variant.span,
                            );
                        }
                    }
                }
            }
            Item::Function(func) if func.visibility == Visibility::Public => {
                let generics: HashSet<String> =
                    func.type_params.iter().map(|tp| tp.name.clone()).collect();

                for param in &func.params {
                    if let Some(annotation) = &param.ty {
                        self.validate_public_type_annotation(
                            annotation,
                            &generics,
                            &format!("Public function '{}' parameter '{}'", func.name, param.name),
                            param.span,
                        );
                    }
                }

                if let Some(ret) = &func.return_type {
                    self.validate_public_type_annotation(
                        ret,
                        &generics,
                        &format!("Public function '{}' return type", func.name),
                        func.span,
                    );
                }
            }
            _ => {}
        }
    }

    fn conversion_hint(&self, actual: &Type, expected: &Type) -> Option<String> {
        match (actual, expected) {
            (Type::Float, Type::Int) => Some(
                "Implicit narrowing from float to int is not allowed; use an explicit conversion.".to_string(),
            ),
            (Type::String, Type::Int | Type::Float | Type::Bool) => Some(
                "Strings cannot be implicitly converted; parse or convert explicitly.".to_string(),
            ),
            (Type::Bool, Type::Int | Type::Float) => Some(
                "Booleans do not implicitly convert to numbers; use a conditional or explicit conversion.".to_string(),
            ),
            (Type::Int, Type::Bool) => Some(
                "Integers do not implicitly convert to booleans; compare against zero or use an explicit helper.".to_string(),
            ),
            _ => None,
        }
    }

    fn types_match(&self, actual: &Type, expected: &Type) -> bool {
        if self.can_auto_promote(actual, expected) {
            return true;
        }

        match (actual, expected) {
            // Tipos idênticos
            (Type::Int, Type::Int) => true,
            (Type::Float, Type::Float) => true,
            (Type::String, Type::String) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Char, Type::Char) => true,
            (Type::Unit, Type::Unit) => true,

            // Structs com mesmo nome
            (Type::Struct { name: n1 }, Type::Struct { name: n2 }) => n1 == n2,

            // Enums com mesmo nome
            (Type::Enum { name: n1, .. }, Type::Enum { name: n2, .. }) => n1 == n2,

            // Unknown aceita qualquer coisa (inferência incompleta)
            (Type::Unknown, _) | (_, Type::Unknown) => true,

            // Self type matches any Struct (will be resolved in context)
            (Type::SelfType, Type::Struct { .. }) | (Type::Struct { .. }, Type::SelfType) => true,
            (Type::SelfType, Type::SelfType) => true,

            // Tuples com mesmo tamanho e tipos compatíveis
            (Type::Tuple { elements: t1 }, Type::Tuple { elements: t2 }) => {
                t1.len() == t2.len()
                    && t1
                        .iter()
                        .zip(t2.iter())
                        .all(|(a, b)| self.types_match(a, b))
            }

            // Generic type parameters are considered compatible with any type during alpha
            (Type::TypeParameter { .. }, _) | (_, Type::TypeParameter { .. }) => true,

            // Arrays com tipos de elemento compatíveis
            (
                Type::Array {
                    element_type: e1,
                    size: s1,
                },
                Type::Array {
                    element_type: e2,
                    size: s2,
                },
            ) => self.types_match(e1, e2) && (s1.is_none() || s2.is_none() || s1 == s2),

            _ => false,
        }
    }

    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        self.types_match(a, b) && self.types_match(b, a)
    }

    fn branch_type_mismatch(&self, types: &[Type]) -> Option<(Type, Type)> {
        let mut reference: Option<Type> = None;

        for ty in types {
            if matches!(ty, Type::Unknown) {
                continue;
            }

            if let Some(ref expected) = reference {
                if !self.types_compatible(expected, ty) {
                    return Some((expected.clone(), ty.clone()));
                }
            } else {
                reference = Some(ty.clone());
            }
        }

        None
    }

    fn first_non_unknown_type(&self, types: &[Type]) -> Option<Type> {
        types
            .iter()
            .find(|ty| !matches!(ty, Type::Unknown))
            .cloned()
    }

    fn infer_block_type(&mut self, block: &Block) -> Type {
        if let Some(last_stmt) = block.statements.last() {
            match &last_stmt.kind {
                StatementKind::Expression(expr) => self.infer_expression_type(expr),
                StatementKind::Return(ret) => {
                    if let Some(value) = &ret.value {
                        self.infer_expression_type(value);
                    }
                    Type::Unknown
                }
                _ => Type::Unit,
            }
        } else {
            Type::Unit
        }
    }

    pub fn analyze_module(&mut self, module: &mut Module) -> Vec<SemanticError> {
        // First pass: collect all declarations (functions, generic structs, generic enums)
        for item in &module.items {
            match item {
                Item::Function(func) => {
                    if self.functions.contains_key(&func.name) {
                        self.error(
                            format!("Function '{}' is already defined", func.name),
                            func.span,
                        );
                    } else {
                        let pushed_generics = self.push_generic_params(&func.type_params);

                        // Extract parameter types
                        let params: Vec<Type> = func
                            .params
                            .iter()
                            .map(|p| self.type_annotation_to_type(&p.ty))
                            .collect();

                        // Extract return type
                        let return_type = self.type_annotation_to_type(&func.return_type);

                        if pushed_generics {
                            self.pop_generic_params();
                        }

                        let signature = FunctionSignature {
                            params,
                            return_type: return_type.clone(),
                            self_kind: None,
                        };

                        self.functions.insert(func.name.clone(), signature);
                        self.declare_symbol(func.name.clone(), func.span, return_type);
                    }
                }
                Item::Struct(struct_def) => {
                    // Build struct metadata and validate duplicate fields
                    let mut fields_map = HashMap::new();
                    for field in &struct_def.fields {
                        if fields_map.contains_key(&field.name) {
                            self.error(
                                format!(
                                    "Field '{}' is duplicated in struct '{}'",
                                    field.name, struct_def.name
                                ),
                                field.span,
                            );
                            continue;
                        }

                        fields_map.insert(
                            field.name.clone(),
                            StructFieldInfo {
                                ty: field.ty.clone(),
                                span: field.span,
                            },
                        );
                    }

                    let struct_info = StructInfo {
                        visibility: struct_def.visibility,
                        type_params: struct_def
                            .type_params
                            .iter()
                            .map(|tp| tp.name.clone())
                            .collect(),
                        fields: fields_map,
                    };

                    if self
                        .struct_infos
                        .insert(struct_def.name.clone(), struct_info)
                        .is_some()
                    {
                        self.error(
                            format!("Struct '{}' is already defined", struct_def.name),
                            struct_def.span,
                        );
                    }

                    // Collect generic structs for type inference
                    if !struct_def.type_params.is_empty() {
                        let type_params = struct_def.type_params.clone();
                        let fields: Vec<(String, crate::ast::TypeAnnotation)> = struct_def
                            .fields
                            .iter()
                            .map(|f| (f.name.clone(), f.ty.clone()))
                            .collect();
                        self.generic_structs
                            .insert(struct_def.name.clone(), (type_params, fields));
                    }
                }
                Item::Enum(enum_def) => {
                    let variant_type_params: Vec<String> = enum_def
                        .type_params
                        .iter()
                        .map(|tp| tp.name.clone())
                        .collect();

                    let mut variants_map = HashMap::new();
                    let mut variant_names = Vec::new();

                    for variant in &enum_def.variants {
                        if variants_map.contains_key(&variant.name) {
                            self.error(
                                format!(
                                    "Variant '{}' is duplicated in enum '{}'",
                                    variant.name, enum_def.name
                                ),
                                variant.span,
                            );
                            continue;
                        }

                        variants_map.insert(
                            variant.name.clone(),
                            EnumVariantInfo {
                                data: variant.data.clone(),
                                span: variant.span,
                            },
                        );
                        variant_names.push(variant.name.clone());
                    }

                    let enum_info = EnumInfo {
                        visibility: enum_def.visibility,
                        type_params: variant_type_params.clone(),
                        variants: variants_map,
                    };

                    if self
                        .enum_infos
                        .insert(enum_def.name.clone(), enum_info)
                        .is_some()
                    {
                        self.error(
                            format!("Enum '{}' is already defined", enum_def.name),
                            enum_def.span,
                        );
                    }

                    // Store variant names for exhaustiveness checking
                    self.enum_definitions
                        .insert(enum_def.name.clone(), variant_names);

                    // Collect generic enums for type inference
                    if !enum_def.type_params.is_empty() {
                        let type_params = enum_def.type_params.clone();
                        let variant_names: Vec<String> =
                            enum_def.variants.iter().map(|v| v.name.clone()).collect();
                        self.generic_enums
                            .insert(enum_def.name.clone(), (type_params, variant_names));
                    }
                }
                _ => {}
            }
        }

        for item in &module.items {
            self.enforce_visibility_rules(item);
        }

        // Second pass: analyze function bodies
        for item in &module.items {
            self.analyze_item(item);
        }

        // Third pass: infer generic type arguments
        for item in &mut module.items {
            self.infer_generic_types_in_item(item);
        }

        // Fourth pass: fill type information in method calls
        for item in &mut module.items {
            self.fill_method_call_types_in_item(item);
        }

        // Return collected errors
        std::mem::take(&mut self.errors)
    }

    fn analyze_item(&mut self, item: &Item) {
        match item {
            Item::Import(_) => {
                // Import analysis would go here
            }
            Item::Function(func) => {
                self.analyze_function(func);
            }
            Item::Struct(_struct) => {
                // Struct metadata is collected during the declaration pass.
            }
            Item::Enum(_enum_def) => {
                // Enum metadata is collected during the declaration pass.
            }
            Item::Impl(impl_block) => {
                self.analyze_impl_block(impl_block);
            }
            Item::Trait(trait_decl) => {
                self.analyze_trait_declaration(trait_decl);
            }
            Item::TraitImpl(trait_impl) => {
                self.analyze_trait_impl(trait_impl);
            }
        }
    }

    fn analyze_trait_impl(&mut self, trait_impl: &crate::ast::TraitImpl) {
        let derived_impl = crate::ast::ImplBlock {
            type_name: trait_impl.type_name.clone(),
            trait_name: Some(trait_impl.trait_name.clone()),
            methods: trait_impl.methods.clone(),
            span: trait_impl.span,
        };

        self.analyze_impl_block(&derived_impl);
    }

    fn analyze_impl_block(&mut self, impl_block: &crate::ast::ImplBlock) {
        let type_param_info = self
            .generic_structs
            .get(&impl_block.type_name)
            .map(|(params, _)| params.clone())
            .or_else(|| {
                self.generic_enums
                    .get(&impl_block.type_name)
                    .map(|(params, _)| params.clone())
            });
        let pushed_generics = if let Some(ref params) = type_param_info {
            self.push_generic_params(params)
        } else {
            false
        };

        // Se for impl Trait for Type, validar que implementa todos os métodos
        if let Some(ref trait_name) = impl_block.trait_name {
            self.validate_trait_impl(impl_block, trait_name);

            // Copiar métodos padrão do trait para o tipo
            self.copy_default_trait_methods(trait_name, &impl_block.type_name, impl_block);
        }

        // Fase 1: Coletar todas as assinaturas dos métodos
        for method in &impl_block.methods {
            // Extrair tipos dos parâmetros
            let mut param_types = Vec::new();
            let mut self_kind = None;
            for param in &method.params {
                if param.is_self {
                    // self parameter - tipo é o do impl block
                    if self_kind.is_some() {
                        self.error(
                            format!(
                                "Method '{}' declares more than one self parameter",
                                method.name
                            ),
                            param.span,
                        );
                    }

                    self_kind = Some(if param.is_reference {
                        SelfParamKind::Reference {
                            mutable: param.is_mutable,
                        }
                    } else {
                        SelfParamKind::Value
                    });
                    param_types.push(Type::Struct {
                        name: impl_block.type_name.clone(),
                    });
                } else {
                    let param_type = self.type_annotation_to_type(&param.type_annotation);
                    param_types.push(param_type);
                }
            }

            let return_type = self.type_annotation_to_type(&method.return_type);

            // Registrar método
            let signature = FunctionSignature {
                params: param_types,
                return_type,
                self_kind,
            };

            let type_methods = self
                .methods
                .entry(impl_block.type_name.clone())
                .or_insert_with(HashMap::new);

            if type_methods
                .insert(method.name.clone(), signature)
                .is_some()
            {
                self.error(
                    format!(
                        "Method '{}' is already defined for type '{}'",
                        method.name, impl_block.type_name
                    ),
                    method.span,
                );
            }
        }

        // Fase 2: Analisar corpos dos métodos
        for method in &impl_block.methods {
            self.current_function = Some(format!("{}::{}", impl_block.type_name, method.name));
            let expected_return = self.type_annotation_to_type(&method.return_type);
            let previous_return = self.current_return_type.replace(expected_return.clone());
            self.push_scope();

            // Declarar parâmetros no escopo
            for param in &method.params {
                let param_type = if param.is_self {
                    Type::Struct {
                        name: impl_block.type_name.clone(),
                    }
                } else {
                    self.type_annotation_to_type(&param.type_annotation)
                };

                if !self.declare_symbol(param.name.clone(), param.span, param_type) {
                    self.error(
                        format!("Parameter '{}' is already declared", param.name),
                        param.span,
                    );
                }
            }

            self.analyze_block(&method.body);
            self.validate_function_block_return(&method.body, &expected_return, method.span);

            self.pop_scope();
            self.current_function = None;
            self.current_return_type = previous_return;
        }

        if pushed_generics {
            self.pop_generic_params();
        }
    }

    /// Analisa declaração de trait e registra assinaturas dos métodos
    fn analyze_trait_declaration(&mut self, trait_decl: &crate::ast::TraitDeclaration) {
        let mut trait_methods = HashMap::new();
        let mut signature_map = HashMap::new();

        // First, inherit methods from parent traits
        for parent_trait_name in &trait_decl.parent_traits {
            if let Some(parent_methods) = self.traits.get(parent_trait_name).cloned() {
                // Add all parent methods to this trait
                for (method_name, method_signature) in parent_methods {
                    trait_methods.insert(method_name, method_signature);
                }
            } else {
                self.error(
                    format!(
                        "Parent trait '{}' is not defined. Traits must be declared before being used as parent traits.",
                        parent_trait_name
                    ),
                    trait_decl.span,
                );
            }

            if let Some(parent_signatures) = self.trait_signatures.get(parent_trait_name).cloned() {
                for (method_name, signature) in parent_signatures {
                    signature_map.insert(method_name, signature);
                }
            }
        }

        // Then add this trait's own methods (can override inherited methods)
        for method in &trait_decl.methods {
            // Converter parâmetros para Type
            let mut param_types = Vec::new();
            let mut self_kind = None;
            let mut parameter_infos = Vec::new();
            for param in &method.params {
                if param.is_self {
                    if self_kind.is_some() {
                        self.error(
                            format!(
                                "Trait method '{}' declares more than one self parameter",
                                method.name
                            ),
                            param.span,
                        );
                    }

                    self_kind = Some(if param.is_reference {
                        SelfParamKind::Reference {
                            mutable: param.is_mutable,
                        }
                    } else {
                        SelfParamKind::Value
                    });
                    // self em trait é genérico - será o tipo que implementa o trait
                    param_types.push(Type::Unknown);
                    parameter_infos.push(ParameterInfo {
                        is_self: true,
                        is_reference: param.is_reference,
                        is_mutable: param.is_mutable,
                        ty: None,
                    });
                } else {
                    let param_type = self.type_annotation_to_type(&param.type_annotation);
                    param_types.push(param_type);
                    parameter_infos.push(ParameterInfo {
                        is_self: false,
                        is_reference: false,
                        is_mutable: false,
                        ty: Self::option_annotation_to_pattern(&param.type_annotation),
                    });
                }
            }

            let return_type = self.type_annotation_to_type(&method.return_type);
            let return_pattern = Self::option_annotation_to_pattern(&method.return_type);

            let signature = FunctionSignature {
                params: param_types,
                return_type,
                self_kind,
            };

            let method_info = TraitMethodInfo {
                signature,
                has_default: method.body.is_some(), // Has default if body is present
                default_body: method.body.clone(),  // Clone the body if present
            };

            if trait_methods
                .insert(method.name.clone(), method_info)
                .is_some()
            {
                self.error(
                    format!(
                        "Method '{}' is already declared in trait '{}'",
                        method.name, trait_decl.name
                    ),
                    method.span,
                );
            }

            signature_map.insert(
                method.name.clone(),
                TraitMethodSignature {
                    params: parameter_infos,
                    return_type: return_pattern,
                    has_default_body: method.body.is_some(),
                },
            );
        }

        // Registrar trait com suas assinaturas
        if self
            .traits
            .insert(trait_decl.name.clone(), trait_methods)
            .is_some()
        {
            self.error(
                format!("Trait '{}' is already defined", trait_decl.name),
                trait_decl.span,
            );
        }

        self.trait_signatures
            .insert(trait_decl.name.clone(), signature_map);
    }

    /// Valida que um impl Trait for Type implementa todos os métodos do trait
    fn validate_trait_impl(&mut self, impl_block: &crate::ast::ImplBlock, trait_name: &str) {
        // Verificar se o trait existe e clonar para evitar borrow conflicts
        let trait_methods = match self.traits.get(trait_name).cloned() {
            Some(methods) => methods,
            None => {
                self.error(
                    format!("Trait '{}' is not defined", trait_name),
                    impl_block.span,
                );
                return;
            }
        };

        let trait_signature_map = self
            .trait_signatures
            .get(trait_name)
            .cloned()
            .unwrap_or_default();

        // Coletar métodos implementados
        let mut implemented_methods = HashMap::new();
        for method in &impl_block.methods {
            // Converter parâmetros para Type
            let mut param_types = Vec::new();
            let mut self_kind = None;
            for param in &method.params {
                if param.is_self {
                    if self_kind.is_some() {
                        self.error(
                            format!(
                                "Method '{}' declares more than one self parameter",
                                method.name
                            ),
                            param.span,
                        );
                    }

                    self_kind = Some(if param.is_reference {
                        SelfParamKind::Reference {
                            mutable: param.is_mutable,
                        }
                    } else {
                        SelfParamKind::Value
                    });
                    param_types.push(Type::Struct {
                        name: impl_block.type_name.clone(),
                    });
                } else {
                    let param_type = self.type_annotation_to_type(&param.type_annotation);
                    param_types.push(param_type);
                }
            }

            let return_type = self.type_annotation_to_type(&method.return_type);

            let signature = FunctionSignature {
                params: param_types,
                return_type,
                self_kind,
            };

            implemented_methods.insert(method.name.clone(), (signature, method.span));
        }

        // Verificar que todos os métodos do trait foram implementados
        for (trait_method_name, trait_method_info) in &trait_methods {
            let expected_signature_repr = trait_signature_map
                .get(trait_method_name)
                .map(|signature| Self::format_trait_signature(trait_method_name, signature));

            match implemented_methods.get(trait_method_name) {
                Some((impl_signature, _span)) => {
                    // Verificar que as assinaturas correspondem
                    // Primeiro parâmetro do trait é Unknown (self genérico), então pulamos
                    // Mas apenas se houver parâmetros (métodos estáticos não têm self)
                    let trait_has_self = trait_method_info.signature.self_kind.is_some();
                    let impl_has_self = impl_signature.self_kind.is_some();

                    if trait_method_info.signature.self_kind != impl_signature.self_kind {
                        self.error(
                            format!(
                                "Method '{}' has incompatible self receiver between trait and implementation",
                                trait_method_name
                            ),
                            impl_block.span,
                        );
                    }

                    let trait_params =
                        if trait_has_self && trait_method_info.signature.params.len() >= 1 {
                            &trait_method_info.signature.params[1..]
                        } else {
                            &trait_method_info.signature.params[..]
                        };

                    let impl_params = if impl_has_self && impl_signature.params.len() >= 1 {
                        &impl_signature.params[1..]
                    } else {
                        &impl_signature.params[..]
                    };

                    if trait_params.len() != impl_params.len() {
                        let mut message = format!(
                            "Method '{}' has wrong number of parameters. Expected {}, found {}",
                            trait_method_name,
                            trait_params.len(),
                            impl_params.len()
                        );

                        if let Some(signature_repr) = &expected_signature_repr {
                            message.push_str(&format!(". Expected {}", signature_repr));
                        }

                        self.error(message, impl_block.span);
                        continue;
                    }

                    // Verificar tipos dos parâmetros
                    for (i, (trait_param, impl_param)) in
                        trait_params.iter().zip(impl_params.iter()).enumerate()
                    {
                        if !self.types_match(impl_param, trait_param) {
                            let mut message = format!(
                                "Method '{}' parameter {} has wrong type. Expected {:?}, found {:?}",
                                trait_method_name,
                                i + 1,
                                trait_param,
                                impl_param
                            );

                            if let Some(signature_repr) = &expected_signature_repr {
                                message.push_str(&format!(" (expected {})", signature_repr));
                            }

                            self.error(message, impl_block.span);
                        }
                    }

                    // Verificar tipo de retorno
                    if !self.types_match(
                        &impl_signature.return_type,
                        &trait_method_info.signature.return_type,
                    ) {
                        let mut message = format!(
                            "Method '{}' has wrong return type. Expected {:?}, found {:?}",
                            trait_method_name,
                            trait_method_info.signature.return_type,
                            impl_signature.return_type
                        );

                        if let Some(signature_repr) = &expected_signature_repr {
                            message.push_str(&format!(" (expected {})", signature_repr));
                        }

                        self.error(message, impl_block.span);
                    }
                }
                None => {
                    // Método não implementado - OK se tem default, erro caso contrário
                    let requires_impl = trait_signature_map
                        .get(trait_method_name)
                        .map(|signature| !signature.has_default_body)
                        .unwrap_or(!trait_method_info.has_default);

                    if requires_impl {
                        let message = if let Some(signature_repr) =
                            trait_signature_map.get(trait_method_name).map(|signature| {
                                Self::format_trait_signature(trait_method_name, signature)
                            }) {
                            format!(
                                "Type '{}' does not implement required trait method '{}' (expected signature: {}; no default implementation)",
                                impl_block.type_name, trait_method_name, signature_repr
                            )
                        } else {
                            format!(
                                "Type '{}' does not implement required trait method '{}' (no default implementation)",
                                impl_block.type_name, trait_method_name
                            )
                        };

                        self.error(message, impl_block.span);
                    }
                }
            }
        }

        // Registrar que este tipo implementa este trait
        self.trait_impls
            .insert((trait_name.to_string(), impl_block.type_name.clone()), true);
    }

    /// Copia métodos padrão do trait para o tipo que o implementa
    fn copy_default_trait_methods(
        &mut self,
        trait_name: &str,
        type_name: &str,
        impl_block: &crate::ast::ImplBlock,
    ) {
        // Obter métodos do trait
        let trait_methods = match self.traits.get(trait_name).cloned() {
            Some(methods) => methods,
            None => return,
        };

        // Obter métodos já implementados
        let implemented_methods: std::collections::HashSet<String> =
            impl_block.methods.iter().map(|m| m.name.clone()).collect();

        // Para cada método do trait com implementação padrão não implementado
        for (method_name, trait_method_info) in trait_methods {
            // Se tem default e não foi implementado
            if trait_method_info.has_default && !implemented_methods.contains(&method_name) {
                // Criar assinatura substituindo self genérico pelo tipo concreto
                let mut concrete_params = Vec::new();
                for (i, param) in trait_method_info.signature.params.iter().enumerate() {
                    if i == 0 && trait_method_info.signature.self_kind.is_some() {
                        // Substituir self genérico pelo tipo concreto
                        concrete_params.push(Type::Struct {
                            name: type_name.to_string(),
                        });
                    } else {
                        concrete_params.push(param.clone());
                    }
                }

                let concrete_signature = FunctionSignature {
                    params: concrete_params,
                    return_type: trait_method_info.signature.return_type.clone(),
                    self_kind: trait_method_info.signature.self_kind,
                };

                // Registrar método no tipo
                let type_methods = self
                    .methods
                    .entry(type_name.to_string())
                    .or_insert_with(HashMap::new);
                type_methods.insert(method_name, concrete_signature);
            }
        }
    }

    fn analyze_function(&mut self, func: &Function) {
        let pushed_generics = self.push_generic_params(&func.type_params);

        self.current_function = Some(func.name.clone());
        let expected_return = self
            .functions
            .get(&func.name)
            .map(|sig| sig.return_type.clone())
            .unwrap_or_else(|| self.type_annotation_to_type(&func.return_type));
        let previous_return = self.current_return_type.replace(expected_return.clone());

        self.push_scope();

        // Declare parameters in function scope
        for param in &func.params {
            let param_type = self.type_annotation_to_type(&param.ty);
            if !self.declare_symbol(param.name.clone(), param.span, param_type) {
                self.error(
                    format!("Parameter '{}' is already declared", param.name),
                    param.span,
                );
            }
        }

        // Analyze function body
        self.analyze_block(&func.body);
        self.validate_function_block_return(&func.body, &expected_return, func.span);

        self.pop_scope();
        if pushed_generics {
            self.pop_generic_params();
        }
        self.current_function = None;
        self.current_return_type = previous_return;
    }

    fn analyze_block(&mut self, block: &Block) {
        self.push_scope();

        for statement in &block.statements {
            self.analyze_statement(statement);
        }

        self.pop_scope();
    }

    fn analyze_statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Let(let_stmt) => {
                // Infer type from value expression or annotation
                let inferred_type = if let Some(ref value) = let_stmt.value {
                    self.infer_expression_type(value)
                } else {
                    self.type_annotation_to_type(&let_stmt.ty)
                };

                // Check if value expression is valid (if present)
                if let Some(ref value) = let_stmt.value {
                    self.analyze_expression(value);
                }

                // Declare the variable with its type
                if !self.declare_symbol(let_stmt.name.clone(), let_stmt.span, inferred_type) {
                    self.error(
                        format!(
                            "Variable '{}' is already declared in this scope",
                            let_stmt.name
                        ),
                        let_stmt.span,
                    );
                }
            }
            StatementKind::Assignment(assign_stmt) => {
                // Analyze the target (lvalue)
                match &assign_stmt.target {
                    crate::ast::LValue::Identifier(name) => {
                        // Check if variable exists
                        if self.lookup_symbol(name).is_none() {
                            self.error(
                                format!("Variable '{}' is not defined", name),
                                assign_stmt.target_span,
                            );
                        }
                    }
                    crate::ast::LValue::IndexAccess { array, index } => {
                        // Analyze array and index expressions
                        self.analyze_expression(array);
                        self.analyze_expression(index);

                        // Check that index is an integer
                        let index_type = self.infer_expression_type(index);
                        if !matches!(index_type, Type::Int | Type::Unknown) {
                            self.error(
                                format!("Array index must be an integer, found {:?}", index_type),
                                assign_stmt.target_span,
                            );
                        }
                    }
                }

                // Analyze the value expression
                self.analyze_expression(&assign_stmt.value);

                let target_type = match &assign_stmt.target {
                    crate::ast::LValue::Identifier(name) => self
                        .lookup_symbol(name)
                        .map(|info| info.ty.clone())
                        .unwrap_or(Type::Unknown),
                    crate::ast::LValue::IndexAccess { array, .. } => {
                        match self.infer_expression_type(array) {
                            Type::Array { element_type, .. } => *element_type,
                            Type::String => Type::Char,
                            _ => Type::Unknown,
                        }
                    }
                };

                let value_type = self.infer_expression_type(&assign_stmt.value);

                if !self.types_match(&value_type, &target_type) {
                    let mut message = format!(
                        "Cannot assign value of type {:?} to target of type {:?}",
                        value_type, target_type
                    );
                    if let Some(hint) = self.conversion_hint(&value_type, &target_type) {
                        message.push_str(" ");
                        message.push_str(&hint);
                    }

                    self.error(message, assign_stmt.value.span);
                }
            }
            StatementKind::Return(ret_stmt) => {
                if self.current_function.is_none() {
                    self.error("Return statement outside of function", ret_stmt.span);
                }

                if let Some(ref value) = ret_stmt.value {
                    self.analyze_expression(value);
                }

                self.check_return_statement(ret_stmt.value.as_ref(), ret_stmt.span);
            }
            StatementKind::Expression(expr) => {
                self.analyze_expression(expr);
            }
            StatementKind::While(while_loop) => {
                self.analyze_expression(&while_loop.condition);
                self.loop_depth += 1;
                self.analyze_block(&while_loop.body);
                self.loop_depth -= 1;
            }
            StatementKind::DoWhile(do_while_loop) => {
                self.loop_depth += 1;
                self.analyze_block(&do_while_loop.body);
                self.loop_depth -= 1;
                self.analyze_expression(&do_while_loop.condition);
            }
            StatementKind::For(for_loop) => {
                self.push_scope();

                // Analyze iterable expression
                self.analyze_expression(&for_loop.iterable);

                // Infer iterator type from iterable expression
                let iterable_type = self.infer_expression_type(&for_loop.iterable);
                let iterator_type = match iterable_type {
                    Type::Array { element_type, .. } => *element_type,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(
                            format!("For-loop iterable must be um array, encontrado {:?}", other),
                            for_loop.span,
                        );
                        Type::Unknown
                    }
                };

                if !self.declare_symbol(
                    for_loop.iterator.clone(),
                    for_loop.span,
                    iterator_type.clone(),
                ) {
                    self.error(
                        format!(
                            "Iterator variable '{}' conflicts with existing declaration",
                            for_loop.iterator
                        ),
                        for_loop.span,
                    );
                }

                // Analyze loop body
                self.loop_depth += 1;
                self.analyze_block(&for_loop.body);
                self.loop_depth -= 1;

                self.pop_scope();
            }
            StatementKind::Loop(loop_stmt) => {
                self.loop_depth += 1;
                self.analyze_block(&loop_stmt.body);
                self.loop_depth -= 1;
            }
            StatementKind::Switch(switch_stmt) => {
                // Analyze the value being switched on
                self.analyze_expression(&switch_stmt.value);

                // Analyze each case
                for case in &switch_stmt.cases {
                    self.analyze_expression(&case.pattern);
                    self.analyze_block(&case.body);
                }

                // Analyze default case if present
                if let Some(ref default_block) = switch_stmt.default {
                    self.analyze_block(default_block);
                }
            }
            StatementKind::Break => {
                if self.loop_depth == 0 {
                    self.error("Break statement outside of loop", statement.span);
                }
            }
            StatementKind::Continue => {
                if self.loop_depth == 0 {
                    self.error("Continue statement outside of loop", statement.span);
                }
            }
        }
    }

    fn infer_expression_type(&mut self, expr: &Expression) -> Type {
        match &expr.kind {
            ExpressionKind::NumberLiteral(num) => {
                if num.contains('.') {
                    Type::Float
                } else {
                    Type::Int
                }
            }
            ExpressionKind::StringLiteral(_) => Type::String,
            ExpressionKind::BoolLiteral(_) => Type::Bool,
            ExpressionKind::Identifier(name) => {
                if let Some(info) = self.lookup_symbol(name) {
                    info.ty.clone()
                } else if let Some(sig) = self.functions.get(name) {
                    sig.return_type.clone()
                } else {
                    Type::Unknown
                }
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_type = self.infer_expression_type(left);
                let right_type = self.infer_expression_type(right);

                use crate::ast::BinaryOperator;
                match operator {
                    BinaryOperator::Add => {
                        if matches!(left_type, Type::String) || matches!(right_type, Type::String) {
                            Type::String
                        } else {
                            self.numeric_result_type(&left_type, &right_type)
                        }
                    }
                    BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => self.numeric_result_type(&left_type, &right_type),
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::Greater
                    | BinaryOperator::LessEqual
                    | BinaryOperator::GreaterEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or => Type::Bool,
                }
            }
            ExpressionKind::Unary { operand, .. } => self.infer_expression_type(operand),
            ExpressionKind::Call { callee, .. } => {
                if let ExpressionKind::Identifier(name) = &callee.kind {
                    if let Some(sig) = self.functions.get(name) {
                        return sig.return_type.clone();
                    }
                }
                Type::Unknown
            }
            ExpressionKind::If {
                then_block,
                elif_blocks,
                else_block,
                ..
            } => {
                let mut branch_types = Vec::new();
                branch_types.push(self.infer_block_type(then_block));
                for (_, elif_block) in elif_blocks {
                    branch_types.push(self.infer_block_type(elif_block));
                }
                branch_types.push(match else_block {
                    Some(block) => self.infer_block_type(block),
                    None => Type::Unit,
                });

                if self.branch_type_mismatch(&branch_types).is_some() {
                    Type::Unknown
                } else {
                    self.first_non_unknown_type(&branch_types)
                        .unwrap_or(Type::Unknown)
                }
            }
            ExpressionKind::Unless {
                then_block,
                else_block,
                ..
            } => {
                let mut branch_types = Vec::new();
                branch_types.push(self.infer_block_type(then_block));
                branch_types.push(match else_block {
                    Some(block) => self.infer_block_type(block),
                    None => Type::Unit,
                });

                if self.branch_type_mismatch(&branch_types).is_some() {
                    Type::Unknown
                } else {
                    self.first_non_unknown_type(&branch_types)
                        .unwrap_or(Type::Unknown)
                }
            }
            ExpressionKind::Grouping(inner) => self.infer_expression_type(inner),
            ExpressionKind::ArrayLiteral { elements } => {
                if elements.is_empty() {
                    Type::Array {
                        element_type: Box::new(Type::Unknown),
                        size: Some(0),
                    }
                } else {
                    let elem_type = self.infer_expression_type(&elements[0]);
                    Type::Array {
                        element_type: Box::new(elem_type),
                        size: Some(elements.len()),
                    }
                }
            }
            ExpressionKind::IndexAccess { array, .. } => {
                let array_type = self.infer_expression_type(array);
                match array_type {
                    Type::Array { element_type, .. } => *element_type,
                    _ => Type::Unknown,
                }
            }
            ExpressionKind::TupleLiteral { elements } => {
                if elements.is_empty() {
                    Type::Tuple { elements: vec![] }
                } else {
                    let element_types: Vec<Type> = elements
                        .iter()
                        .map(|e| self.infer_expression_type(e))
                        .collect();
                    Type::Tuple {
                        elements: element_types,
                    }
                }
            }
            ExpressionKind::TupleAccess { tuple, index } => {
                let tuple_type = self.infer_expression_type(tuple);
                match tuple_type {
                    Type::Tuple { elements } => {
                        if *index < elements.len() {
                            elements[*index].clone()
                        } else {
                            Type::Unknown
                        }
                    }
                    _ => Type::Unknown,
                }
            }
            ExpressionKind::StructLiteral { name, .. } => {
                if self.struct_infos.contains_key(name) {
                    Type::Struct { name: name.clone() }
                } else {
                    Type::Unknown
                }
            }
            ExpressionKind::FieldAccess { object, field } => {
                let object_type = self.infer_expression_type(object);
                match object_type {
                    Type::Struct { name } => {
                        if let Some(expected_ann) = self
                            .struct_infos
                            .get(&name)
                            .and_then(|info| info.fields.get(field))
                            .map(|field_info| field_info.ty.clone())
                        {
                            self.type_annotation_to_type(&Some(expected_ann))
                        } else {
                            Type::Unknown
                        }
                    }
                    _ => Type::Unknown,
                }
            }
            ExpressionKind::EnumVariant { enum_name, .. } => {
                if self.enum_infos.contains_key(enum_name) {
                    Type::Enum {
                        name: enum_name.clone(),
                    }
                } else {
                    Type::Unknown
                }
            }
            ExpressionKind::Match { scrutinee: _, arms } => {
                if arms.is_empty() {
                    return Type::Unknown;
                }

                let arm_types: Vec<Type> = arms
                    .iter()
                    .map(|arm| self.infer_expression_type(&arm.body))
                    .collect();

                if self.branch_type_mismatch(&arm_types).is_some() {
                    Type::Unknown
                } else {
                    self.first_non_unknown_type(&arm_types)
                        .unwrap_or(Type::Unknown)
                }
            }
            ExpressionKind::MethodCall {
                object,
                method_name,
                ..
            } => {
                let obj_type = self.infer_expression_type(object);
                let type_name = match &obj_type {
                    Type::Struct { name } => Some(name.clone()),
                    Type::Enum { name, .. } => Some(name.clone()),
                    _ => None,
                };

                if let Some(type_name) = type_name {
                    if let Some(type_methods) = self.methods.get(&type_name) {
                        if let Some(signature) = type_methods.get(method_name) {
                            return signature.return_type.clone();
                        }
                    }
                }

                Type::Unknown
            }
        }
    }

    fn analyze_expression(&mut self, expr: &Expression) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                // Check if identifier is declared
                if self.lookup_symbol(name).is_none() && !self.functions.contains_key(name) {
                    self.error(
                        format!("Undefined variable or function '{}'", name),
                        expr.span,
                    );
                }
            }
            ExpressionKind::NumberLiteral(_)
            | ExpressionKind::StringLiteral(_)
            | ExpressionKind::BoolLiteral(_) => {
                // Literals are always valid
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                self.analyze_expression(left);
                self.analyze_expression(right);

                // Type check binary operations
                let left_type = self.infer_expression_type(left);
                let right_type = self.infer_expression_type(right);

                use crate::ast::BinaryOperator;
                match operator {
                    BinaryOperator::Add => {
                        // Add supports both numeric types and string concatenation
                        let is_string_concat =
                            matches!(left_type, Type::String) || matches!(right_type, Type::String);

                        if is_string_concat {
                            // String concatenation - both operands must be strings
                            if !matches!(left_type, Type::String | Type::Unknown) {
                                self.error(
                                    format!(
                                        "Cannot concatenate non-string type {:?} with string",
                                        left_type
                                    ),
                                    left.span,
                                );
                            }
                            if !matches!(right_type, Type::String | Type::Unknown) {
                                self.error(
                                    format!(
                                        "Cannot concatenate string with non-string type {:?}",
                                        right_type
                                    ),
                                    right.span,
                                );
                            }
                        } else {
                            // Numeric addition
                            if !matches!(left_type, Type::Int | Type::Float | Type::Unknown) {
                                self.error(
                                    format!("Left operand of arithmetic operation must be numeric, found {:?}", left_type),
                                    left.span,
                                );
                            }
                            if !matches!(right_type, Type::Int | Type::Float | Type::Unknown) {
                                self.error(
                                    format!("Right operand of arithmetic operation must be numeric, found {:?}", right_type),
                                    right.span,
                                );
                            }
                        }
                    }
                    BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => {
                        // Arithmetic operations require numeric types
                        if !matches!(left_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!("Left operand of arithmetic operation must be numeric, found {:?}", left_type),
                                left.span,
                            );
                        }
                        if !matches!(right_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!("Right operand of arithmetic operation must be numeric, found {:?}", right_type),
                                right.span,
                            );
                        }
                        if !self.numeric_types_can_interact(&left_type, &right_type) {
                            self.error(
                                format!(
                                    "Type mismatch in arithmetic operation: {:?} and {:?}",
                                    left_type, right_type
                                ),
                                expr.span,
                            );
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        // Equality can compare any types, but they should match
                        if left_type != Type::Unknown
                            && right_type != Type::Unknown
                            && left_type != right_type
                            && !self.numeric_types_can_interact(&left_type, &right_type)
                        {
                            self.error(
                                format!(
                                    "Type mismatch in equality comparison: {:?} and {:?}",
                                    left_type, right_type
                                ),
                                expr.span,
                            );
                        }
                    }
                    BinaryOperator::Less
                    | BinaryOperator::Greater
                    | BinaryOperator::LessEqual
                    | BinaryOperator::GreaterEqual => {
                        // Comparison requires numeric types
                        if !matches!(left_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!(
                                    "Left operand of comparison must be numeric, found {:?}",
                                    left_type
                                ),
                                left.span,
                            );
                        }
                        if !matches!(right_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!(
                                    "Right operand of comparison must be numeric, found {:?}",
                                    right_type
                                ),
                                right.span,
                            );
                        }
                    }
                    BinaryOperator::And | BinaryOperator::Or => {
                        // Logical operations require boolean types
                        if !matches!(left_type, Type::Bool | Type::Unknown) {
                            self.error(
                                format!(
                                    "Left operand of logical operation must be boolean, found {:?}",
                                    left_type
                                ),
                                left.span,
                            );
                        }
                        if !matches!(right_type, Type::Bool | Type::Unknown) {
                            self.error(
                                format!("Right operand of logical operation must be boolean, found {:?}", right_type),
                                right.span,
                            );
                        }
                    }
                }
            }
            ExpressionKind::Unary { operand, .. } => {
                self.analyze_expression(operand);
            }
            ExpressionKind::Call { callee, arguments } => {
                // Check if function exists and validate argument types
                if let ExpressionKind::Identifier(name) = &callee.kind {
                    if let Some(signature) = self.functions.get(name).cloned() {
                        // Validate number of arguments
                        if arguments.len() != signature.params.len() {
                            self.error(
                                format!(
                                    "Function '{}' expects {} arguments, but {} were provided",
                                    name,
                                    signature.params.len(),
                                    arguments.len()
                                ),
                                expr.span,
                            );
                        } else {
                            // Validate argument types
                            for (i, (arg, expected_type)) in
                                arguments.iter().zip(&signature.params).enumerate()
                            {
                                let arg_type = self.infer_expression_type(arg);
                                if arg_type != Type::Unknown
                                    && *expected_type != Type::Unknown
                                    && arg_type != *expected_type
                                {
                                    self.error(
                                        format!(
                                            "Argument {} of function '{}' has type {:?}, expected {:?}",
                                            i + 1,
                                            name,
                                            arg_type,
                                            expected_type
                                        ),
                                        arg.span,
                                    );
                                }
                            }
                        }
                    } else if self.lookup_symbol(name).is_none() {
                        self.error(format!("Undefined function '{}'", name), callee.span);
                    }
                } else {
                    self.analyze_expression(callee);
                }

                // Analyze arguments
                for arg in arguments {
                    self.analyze_expression(arg);
                }
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.analyze_expression(condition);
                self.analyze_block(then_block);

                for (elif_cond, elif_body) in elif_blocks {
                    self.analyze_expression(elif_cond);
                    self.analyze_block(elif_body);
                }

                if let Some(ref else_body) = else_block {
                    self.analyze_block(else_body);
                }

                let mut branch_types = Vec::new();
                branch_types.push(self.infer_block_type(then_block));
                for (_, elif_body) in elif_blocks {
                    branch_types.push(self.infer_block_type(elif_body));
                }
                branch_types.push(match else_block {
                    Some(block) => self.infer_block_type(block),
                    None => Type::Unit,
                });

                if let Some((expected, found)) = self.branch_type_mismatch(&branch_types) {
                    self.error(
                        format!(
                            "Incompatible branch types in if expression: expected {:?}, found {:?}",
                            expected, found
                        ),
                        expr.span,
                    );
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                self.analyze_expression(condition);
                self.analyze_block(then_block);

                if let Some(ref else_body) = else_block {
                    self.analyze_block(else_body);
                }

                let mut branch_types = Vec::new();
                branch_types.push(self.infer_block_type(then_block));
                branch_types.push(match else_block {
                    Some(block) => self.infer_block_type(block),
                    None => Type::Unit,
                });

                if let Some((expected, found)) = self.branch_type_mismatch(&branch_types) {
                    self.error(
                        format!(
                            "Incompatible branch types in unless expression: expected {:?}, found {:?}",
                            expected, found
                        ),
                        expr.span,
                    );
                }
            }
            ExpressionKind::Grouping(inner) => {
                self.analyze_expression(inner);
            }
            ExpressionKind::ArrayLiteral { elements } => {
                // Analyze all elements
                for element in elements {
                    self.analyze_expression(element);
                }

                // Check that all elements have the same type
                if !elements.is_empty() {
                    let first_type = self.infer_expression_type(&elements[0]);
                    for (i, element) in elements.iter().enumerate().skip(1) {
                        let elem_type = self.infer_expression_type(element);
                        if first_type != Type::Unknown
                            && elem_type != Type::Unknown
                            && first_type != elem_type
                        {
                            self.error(
                                format!(
                                    "Array element {} has type {:?}, expected {:?}",
                                    i, elem_type, first_type
                                ),
                                element.span,
                            );
                        }
                    }
                }
            }
            ExpressionKind::IndexAccess { array, index } => {
                self.analyze_expression(array);
                self.analyze_expression(index);

                // Check that index is an integer
                let index_type = self.infer_expression_type(index);
                if !matches!(index_type, Type::Int | Type::Unknown) {
                    self.error(
                        format!("Array index must be an integer, found {:?}", index_type),
                        index.span,
                    );
                }

                // Check that array is actually an array
                let array_type = self.infer_expression_type(array);
                if !matches!(array_type, Type::Array { .. } | Type::Unknown) {
                    self.error(
                        format!("Cannot index into non-array type {:?}", array_type),
                        array.span,
                    );
                }
            }
            ExpressionKind::TupleLiteral { elements } => {
                // Analyze all elements
                for element in elements {
                    self.analyze_expression(element);
                }
            }
            ExpressionKind::TupleAccess { tuple, index } => {
                self.analyze_expression(tuple);

                // Check that tuple is actually a tuple
                let tuple_type = self.infer_expression_type(tuple);
                match tuple_type {
                    Type::Tuple { elements } => {
                        if *index >= elements.len() {
                            self.error(
                                format!(
                                    "Tuple index {} out of bounds (tuple has {} elements)",
                                    index,
                                    elements.len()
                                ),
                                tuple.span,
                            );
                        }
                    }
                    Type::Unknown => {
                        // Can't validate, but don't error
                    }
                    _ => {
                        self.error(
                            format!(
                                "Cannot access tuple element on non-tuple type {:?}",
                                tuple_type
                            ),
                            tuple.span,
                        );
                    }
                }
            }
            ExpressionKind::StructLiteral {
                name,
                type_args,
                fields,
            } => {
                // Validate struct exists
                let struct_info = match self.struct_infos.get(name).cloned() {
                    Some(info) => info,
                    None => {
                        self.error(format!("Struct '{}' is not defined", name), expr.span);
                        // Still analyze field expressions to surface nested errors
                        for (_, field_value) in fields {
                            self.analyze_expression(field_value);
                        }
                        return;
                    }
                };

                // Validate type argument arity when explicitly provided
                let expected_type_arg_count = struct_info.type_params.len();
                if !type_args.is_empty() {
                    if expected_type_arg_count == 0 {
                        self.error(
                            format!(
                                "Struct '{}' does not accept type arguments, but {} were provided",
                                name,
                                type_args.len()
                            ),
                            expr.span,
                        );
                    } else if type_args.len() != expected_type_arg_count {
                        self.error(
                            format!(
                                "Struct '{}' expects {} type argument(s), but {} were provided",
                                name,
                                expected_type_arg_count,
                                type_args.len()
                            ),
                            expr.span,
                        );
                    }
                }

                let mut provided_fields = HashSet::new();

                for (field_name, field_value) in fields {
                    self.analyze_expression(field_value);

                    if !provided_fields.insert(field_name.clone()) {
                        self.error(
                            format!(
                                "Field '{}' is specified multiple times in struct literal '{}'",
                                field_name, name
                            ),
                            field_value.span,
                        );
                        continue;
                    }

                    if let Some(expected_field) = struct_info.fields.get(field_name) {
                        let value_type = self.infer_expression_type(field_value);
                        let expected_type =
                            self.type_annotation_to_type(&Some(expected_field.ty.clone()));

                        if !self.types_match(&value_type, &expected_type) {
                            let mut message = format!(
                                "Field '{}' in struct '{}' has type {:?}, but {:?} was expected",
                                field_name, name, value_type, expected_type
                            );
                            if let Some(hint) = self.conversion_hint(&value_type, &expected_type) {
                                message.push_str(" ");
                                message.push_str(&hint);
                            }

                            self.error(message, field_value.span);
                        }
                    } else {
                        self.error(
                            format!("Struct '{}' has no field named '{}'", name, field_name),
                            field_value.span,
                        );
                    }
                }

                for expected_field_name in struct_info.fields.keys() {
                    if !provided_fields.contains(expected_field_name) {
                        self.error(
                            format!(
                                "Struct literal for '{}' is missing field '{}'",
                                name, expected_field_name
                            ),
                            expr.span,
                        );
                    }
                }
            }
            ExpressionKind::FieldAccess { object, field } => {
                self.analyze_expression(object);

                let object_type = self.infer_expression_type(object);
                match object_type {
                    Type::Struct { name } => {
                        if let Some(struct_info) = self.struct_infos.get(&name) {
                            if !struct_info.fields.contains_key(field) {
                                self.error(
                                    format!("Struct '{}' has no field named '{}'", name, field),
                                    expr.span,
                                );
                            }
                        } else {
                            self.error(format!("Struct '{}' is not defined", name), expr.span);
                        }
                    }
                    Type::Unknown => {
                        // Cannot validate without type information
                    }
                    _ => {
                        self.error(
                            format!(
                                "Cannot access field '{}' on non-struct type {:?}",
                                field, object_type
                            ),
                            expr.span,
                        );
                    }
                }
            }
            ExpressionKind::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
                ..
            } => {
                if let Some(args) = data {
                    for arg in args {
                        self.analyze_expression(arg);
                    }
                }

                let enum_info = match self.enum_infos.get(enum_name).cloned() {
                    Some(info) => info,
                    None => {
                        self.error(format!("Enum '{}' is not defined", enum_name), expr.span);
                        return;
                    }
                };

                let expected_type_arg_count = enum_info.type_params.len();
                if !type_args.is_empty() {
                    if expected_type_arg_count == 0 {
                        self.error(
                            format!(
                                "Enum '{}' does not accept type arguments, but {} were provided",
                                enum_name,
                                type_args.len()
                            ),
                            expr.span,
                        );
                    } else if type_args.len() != expected_type_arg_count {
                        self.error(
                            format!(
                                "Enum '{}' expects {} type argument(s), but {} were provided",
                                enum_name,
                                expected_type_arg_count,
                                type_args.len()
                            ),
                            expr.span,
                        );
                    }
                }

                let variant_info = match enum_info.variants.get(variant_name).cloned() {
                    Some(info) => info,
                    None => {
                        self.error(
                            format!(
                                "Enum '{}' has no variant named '{}'",
                                enum_name, variant_name
                            ),
                            expr.span,
                        );
                        return;
                    }
                };

                match (&variant_info.data, data) {
                    (Some(expected_params), Some(actual_args)) => {
                        if expected_params.len() != actual_args.len() {
                            self.error(
                                format!(
                                    "Variant '{}::{}' expects {} value(s), but {} were provided",
                                    enum_name,
                                    variant_name,
                                    expected_params.len(),
                                    actual_args.len()
                                ),
                                expr.span,
                            );
                        }

                        for (idx, (expected_ann, arg_expr)) in
                            expected_params.iter().zip(actual_args.iter()).enumerate()
                        {
                            let arg_type = self.infer_expression_type(arg_expr);
                            let expected_type =
                                self.type_annotation_to_type(&Some(expected_ann.clone()));

                            if !self.types_match(&arg_type, &expected_type) {
                                self.error(
                                    format!(
                                        "Argument {} for variant '{}::{}' has type {:?}, but {:?} was expected",
                                        idx + 1,
                                        enum_name,
                                        variant_name,
                                        arg_type,
                                        expected_type
                                    ),
                                    arg_expr.span,
                                );
                            }
                        }
                    }
                    (Some(expected_params), None) => {
                        self.error(
                            format!(
                                "Variant '{}::{}' expects {} value(s)",
                                enum_name,
                                variant_name,
                                expected_params.len()
                            ),
                            expr.span,
                        );
                    }
                    (None, Some(actual_args)) => {
                        if !actual_args.is_empty() {
                            self.error(
                                format!(
                                    "Variant '{}::{}' does not take any values",
                                    enum_name, variant_name
                                ),
                                expr.span,
                            );
                        }
                    }
                    (None, None) => {}
                }
            }
            ExpressionKind::Match { scrutinee, arms } => {
                self.analyze_expression(scrutinee);
                let scrutinee_type = self.infer_expression_type(scrutinee);

                // Verificar exhaustiveness
                self.check_match_exhaustiveness(&scrutinee_type, arms, expr.span);

                let mut arm_result_types = Vec::new();
                for arm in arms {
                    // Criar novo escopo para o arm
                    self.push_scope();

                    self.validate_pattern_against_type(&arm.pattern, &scrutinee_type, expr.span);
                    // Registrar variáveis do pattern
                    self.register_pattern_bindings(&arm.pattern);
                    self.bind_pattern_types(&arm.pattern, &scrutinee_type);

                    // Analisar corpo do arm
                    self.analyze_expression(&arm.body);
                    arm_result_types.push(self.infer_expression_type(&arm.body));

                    // Sair do escopo
                    self.pop_scope();
                }

                if let Some((expected, found)) = self.branch_type_mismatch(&arm_result_types) {
                    self.error(
                        format!(
                            "Match arms must return compatible types; expected {:?}, found {:?}",
                            expected, found
                        ),
                        expr.span,
                    );
                }
            }
            ExpressionKind::MethodCall {
                object,
                method_name,
                arguments,
                type_name: _,
            } => {
                // Analisar objeto
                self.analyze_expression(object);

                // Analisar argumentos
                for arg in arguments {
                    self.analyze_expression(arg);
                }

                // Verificar se método existe para o tipo do objeto
                let obj_type = self.infer_expression_type(object);

                if let Type::TypeParameter { name } = &obj_type {
                    match self.trait_method_signature_for_type_param(name, method_name) {
                        Some((signature, _trait_name)) => {
                            self.validate_method_call_signature(
                                method_name,
                                &signature,
                                &obj_type,
                                arguments,
                                expr.span,
                            );
                        }
                        None => match self.get_generic_bounds(name) {
                            Some(bounds) if !bounds.is_empty() => {
                                self.error(
                                        format!(
                                            "Method '{}' is not provided by trait bounds ({}) on type parameter '{}'",
                                            method_name,
                                            bounds.join(", "),
                                            name
                                        ),
                                        expr.span,
                                    );
                            }
                            _ => {
                                self.error(
                                        format!(
                                            "Type parameter '{}' must be constrained by a trait that defines method '{}'",
                                            name, method_name
                                        ),
                                        expr.span,
                                    );
                            }
                        },
                    }
                    return;
                }

                // Extrair nome do tipo
                let type_name = match &obj_type {
                    Type::Struct { name } => Some(name.clone()),
                    Type::Enum { name, .. } => Some(name.clone()),
                    Type::Unknown => None,
                    _ => {
                        self.error(
                            format!(
                                "Cannot call method '{}' on type '{:?}'",
                                method_name, obj_type
                            ),
                            expr.span,
                        );
                        None
                    }
                };

                // Se conseguimos extrair o tipo, verificar se método existe
                if let Some(type_name) = &type_name {
                    let method_signature = self
                        .methods
                        .get(type_name)
                        .and_then(|methods| methods.get(method_name).cloned());

                    let signature = if let Some(sig) = method_signature {
                        Some(sig)
                    } else {
                        let mut found_signature = None;
                        for ((trait_name, impl_type), _) in &self.trait_impls {
                            if impl_type == type_name {
                                if let Some(trait_methods) = self.traits.get(trait_name) {
                                    if let Some(trait_method_info) = trait_methods.get(method_name)
                                    {
                                        if trait_method_info.has_default {
                                            let mut params =
                                                trait_method_info.signature.params.clone();
                                            if trait_method_info.signature.self_kind.is_some()
                                                && !params.is_empty()
                                            {
                                                params[0] = Type::Struct {
                                                    name: type_name.clone(),
                                                };
                                            }

                                            found_signature = Some(FunctionSignature {
                                                params,
                                                return_type: trait_method_info
                                                    .signature
                                                    .return_type
                                                    .clone(),
                                                self_kind: trait_method_info.signature.self_kind,
                                            });
                                            break;
                                        }
                                    }
                                }
                            }
                            if found_signature.is_some() {
                                break;
                            }
                        }
                        found_signature
                    };

                    if let Some(signature) = signature {
                        self.validate_method_call_signature(
                            method_name,
                            &signature,
                            &obj_type,
                            arguments,
                            expr.span,
                        );
                    } else if self.methods.contains_key(type_name) {
                        self.error(
                            format!(
                                "Method '{}' not found for type '{}'",
                                method_name, type_name
                            ),
                            expr.span,
                        );
                    } else {
                        self.error(
                            format!("No methods defined for type '{}'", type_name),
                            expr.span,
                        );
                    }
                }
            }
        }
    }

    /// Registra variáveis bound por um pattern no escopo atual
    fn register_pattern_bindings(&mut self, pattern: &Pattern) {
        use crate::ast::Pattern;

        match pattern {
            Pattern::Wildcard => {
                // Não cria bindings
            }
            Pattern::Identifier(name) => {
                // Registra a variável no escopo atual
                // Tipo será inferido posteriormente
                if let Some(scope) = self.symbols.last_mut() {
                    scope.insert(
                        name.clone(),
                        SymbolInfo {
                            span: Span::dummy(),
                            ty: Type::Unknown,
                        },
                    );
                }
            }
            Pattern::Literal(_) => {
                // Não cria bindings
            }
            Pattern::EnumVariant {
                enum_name: _,
                variant_name: _,
                data,
                ..
            } => {
                // Se houver sub-patterns, registrar recursivamente
                if let Some(sub_patterns) = data {
                    for sub_pattern in sub_patterns {
                        self.register_pattern_bindings(sub_pattern);
                    }
                }
            }
        }
    }

    /// Ensure match arm patterns are compatible with the scrutinee type before binding names.
    fn validate_pattern_against_type(
        &mut self,
        pattern: &Pattern,
        scrutinee_type: &Type,
        match_span: Span,
    ) {
        use crate::ast::Pattern;

        match pattern {
            Pattern::Wildcard | Pattern::Identifier(_) => {}
            Pattern::Literal(expr) => {
                if matches!(scrutinee_type, Type::Unknown) {
                    return;
                }

                let literal_type = self.infer_expression_type(expr);
                if matches!(literal_type, Type::Unknown) {
                    return;
                }

                if !self.types_match(&literal_type, scrutinee_type) {
                    self.error(
                        format!(
                            "Pattern literal of type {:?} cannot match value of type {:?}",
                            literal_type, scrutinee_type
                        ),
                        expr.span,
                    );
                }
            }
            Pattern::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
            } => {
                let enum_info = match self.enum_infos.get(enum_name).cloned() {
                    Some(info) => info,
                    None => {
                        self.error(format!("Enum '{}' is not defined", enum_name), match_span);
                        return;
                    }
                };

                if !type_args.is_empty() {
                    let expected_args = enum_info.type_params.len();
                    if expected_args == 0 {
                        self.error(
                            format!(
                                "Enum '{}' does not accept type arguments, but {} were provided in pattern",
                                enum_name,
                                type_args.len()
                            ),
                            match_span,
                        );
                    } else if type_args.len() != expected_args {
                        self.error(
                            format!(
                                "Enum '{}' pattern expects {} type argument(s), but {} were provided",
                                enum_name,
                                expected_args,
                                type_args.len()
                            ),
                            match_span,
                        );
                    }
                }

                match scrutinee_type {
                    Type::Enum { name } if name == enum_name => {}
                    Type::Unknown => {}
                    Type::Enum { name } => {
                        self.error(
                            format!(
                                "Pattern '{}::{}' cannot match enum value of type '{}'",
                                enum_name, variant_name, name
                            ),
                            match_span,
                        );
                    }
                    other => {
                        self.error(
                            format!(
                                "Pattern '{}::{}' cannot match value of type {:?}",
                                enum_name, variant_name, other
                            ),
                            match_span,
                        );
                        return;
                    }
                }

                let variant_info = match enum_info.variants.get(variant_name).cloned() {
                    Some(info) => info,
                    None => {
                        self.error(
                            format!(
                                "Enum '{}' has no variant named '{}'",
                                enum_name, variant_name
                            ),
                            match_span,
                        );
                        return;
                    }
                };

                match (&variant_info.data, data) {
                    (Some(expected_types), Some(sub_patterns)) => {
                        if expected_types.len() != sub_patterns.len() {
                            self.error(
                                format!(
                                    "Pattern for variant '{}::{}' has {} field(s), but {} were expected",
                                    enum_name,
                                    variant_name,
                                    sub_patterns.len(),
                                    expected_types.len()
                                ),
                                match_span,
                            );
                            return;
                        }

                        for (sub_pattern, expected_ann) in
                            sub_patterns.iter().zip(expected_types.iter())
                        {
                            let expected_ty =
                                self.type_annotation_to_type(&Some(expected_ann.clone()));
                            self.validate_pattern_against_type(
                                sub_pattern,
                                &expected_ty,
                                match_span,
                            );
                        }
                    }
                    (Some(expected_types), None) => {
                        self.error(
                            format!(
                                "Pattern for variant '{}::{}' is missing {} field(s)",
                                enum_name,
                                variant_name,
                                expected_types.len()
                            ),
                            match_span,
                        );
                    }
                    (None, Some(sub_patterns)) if !sub_patterns.is_empty() => {
                        self.error(
                            format!(
                                "Variant '{}::{}' does not contain any data",
                                enum_name, variant_name
                            ),
                            match_span,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    fn infer_pattern_type(&mut self, pattern: &Pattern) -> Option<Type> {
        use crate::ast::Pattern;

        match pattern {
            Pattern::Wildcard => None,
            Pattern::Identifier(_) => Some(Type::Unknown),
            Pattern::Literal(expr) => Some(self.infer_expression_type(expr)),
            Pattern::EnumVariant {
                enum_name,
                variant_name,
                ..
            } => {
                if let Some(enum_info) = self.enum_infos.get(enum_name) {
                    if enum_info.variants.contains_key(variant_name) {
                        return Some(Type::Enum {
                            name: enum_name.clone(),
                        });
                    }
                }
                None
            }
        }
    }

    fn bind_pattern_types(&mut self, pattern: &Pattern, ty: &Type) {
        use crate::ast::Pattern;

        let mut effective_type = ty.clone();
        if matches!(effective_type, Type::Unknown) {
            if let Some(inferred) = self.infer_pattern_type(pattern) {
                effective_type = inferred;
            }
        }

        match pattern {
            Pattern::Wildcard => {}
            Pattern::Identifier(name) => {
                if let Some(scope) = self.symbols.last_mut() {
                    if let Some(info) = scope.get_mut(name) {
                        info.ty = effective_type.clone();
                    }
                }
            }
            Pattern::Literal(_) => {}
            Pattern::EnumVariant {
                enum_name,
                variant_name,
                data,
                ..
            } => {
                let enum_type = match &effective_type {
                    Type::Enum { name } => name.clone(),
                    _ => {
                        if let Some(Type::Enum { name }) = self.infer_pattern_type(pattern) {
                            name
                        } else {
                            return;
                        }
                    }
                };

                if enum_type != *enum_name {
                    return;
                }

                let Some(enum_info) = self.enum_infos.get(enum_name) else {
                    return;
                };
                let Some(variant_info) = enum_info.variants.get(variant_name) else {
                    return;
                };

                if let (Some(sub_patterns), Some(field_types)) = (&data, &variant_info.data) {
                    if sub_patterns.len() != field_types.len() {
                        return;
                    }

                    let inferred_field_types: Vec<Type> = field_types
                        .iter()
                        .map(|ann| self.type_annotation_to_type(&Some(ann.clone())))
                        .collect();

                    for (sub_pattern, field_type) in
                        sub_patterns.iter().zip(inferred_field_types.iter())
                    {
                        self.bind_pattern_types(sub_pattern, field_type);
                    }
                }
            }
        }
    }

    fn check_return_statement(&mut self, value: Option<&Expression>, span: Span) {
        let expected = match self.current_return_type.as_ref() {
            Some(ty) => ty.clone(),
            None => return,
        };

        match value {
            Some(expr) => {
                if matches!(expected, Type::Unit) {
                    self.error(
                        "Return statement in function with no return type",
                        expr.span,
                    );
                    return;
                }

                let actual = self.infer_expression_type(expr);
                if matches!(actual, Type::Unknown) || matches!(expected, Type::Unknown) {
                    return;
                }

                if !self.types_match(&actual, &expected) {
                    let mut message = format!(
                        "Return type mismatch: expected {:?}, found {:?}",
                        expected, actual
                    );
                    if let Some(hint) = self.conversion_hint(&actual, &expected) {
                        message.push_str(" ");
                        message.push_str(&hint);
                    }

                    self.error(message, expr.span);
                }
            }
            None => {
                if !matches!(expected, Type::Unit | Type::Unknown) {
                    self.error(
                        format!("Return statement missing value of type {:?}", expected),
                        span,
                    );
                }
            }
        }
    }

    fn block_guaranteed_return(&self, block: &Block) -> bool {
        if block.statements.is_empty() {
            return false;
        }

        for (index, statement) in block.statements.iter().enumerate() {
            let is_last = index + 1 == block.statements.len();
            if self.statement_guaranteed_return(statement, is_last) {
                return true;
            }
        }

        false
    }

    fn statement_guaranteed_return(&self, statement: &Statement, is_last: bool) -> bool {
        use crate::ast::StatementKind;

        match &statement.kind {
            StatementKind::Return(_) => true,
            StatementKind::Expression(expr) if is_last => self.expression_guaranteed_return(expr),
            StatementKind::Loop(loop_stmt) if is_last => {
                self.block_guaranteed_return(&loop_stmt.body)
            }
            _ => false,
        }
    }

    fn expression_guaranteed_return(&self, expression: &Expression) -> bool {
        use crate::ast::ExpressionKind;

        match &expression.kind {
            ExpressionKind::If {
                then_block,
                elif_blocks,
                else_block,
                ..
            } => {
                let then_returns = self.block_guaranteed_return(then_block);
                let elif_returns = elif_blocks
                    .iter()
                    .all(|(_, block)| self.block_guaranteed_return(block));
                let else_returns = else_block
                    .as_ref()
                    .map(|block| self.block_guaranteed_return(block))
                    .unwrap_or(false);

                then_returns && elif_returns && else_returns
            }
            ExpressionKind::Unless {
                then_block,
                else_block,
                ..
            } => {
                let then_returns = self.block_guaranteed_return(then_block);
                let else_returns = else_block
                    .as_ref()
                    .map(|block| self.block_guaranteed_return(block))
                    .unwrap_or(false);

                then_returns && else_returns
            }
            ExpressionKind::Match { arms, .. } => {
                !arms.is_empty()
                    && arms
                        .iter()
                        .all(|arm| self.expression_guaranteed_return(&arm.body))
            }
            _ => true,
        }
    }

    fn validate_function_block_return(&mut self, body: &Block, expected: &Type, span: Span) {
        match expected {
            Type::Unknown => return,
            Type::Unit => {
                let block_type = self.infer_block_type(body);
                if !matches!(block_type, Type::Unit | Type::Unknown) {
                    self.error(
                        format!(
                            "Function declared with no return type but final expression has type {:?}",
                            block_type
                        ),
                        body.span,
                    );
                }
            }
            expected_type => {
                if !self.block_guaranteed_return(body) {
                    self.error(
                        format!(
                            "Function declared to return {:?} may exit without returning a value",
                            expected_type
                        ),
                        span,
                    );
                }

                let block_type = self.infer_block_type(body);
                match block_type {
                    Type::Unknown => {}
                    Type::Unit => {
                        self.error(
                            format!("Function must return value of type {:?}", expected_type),
                            span,
                        );
                    }
                    actual => {
                        if !self.types_match(&actual, expected_type) {
                            self.error(
                                format!(
                                    "Function final expression has type {:?}, expected {:?}",
                                    actual, expected_type
                                ),
                                body.span,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Verifica se um match expression é exhaustivo
    fn check_match_exhaustiveness(
        &mut self,
        scrutinee_type: &Type,
        arms: &[crate::ast::MatchArm],
        span: Span,
    ) {
        use crate::ast::{ExpressionKind, Pattern};

        // Se tem wildcard ou identifier, é automaticamente exhaustivo
        let has_catch_all = arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard | Pattern::Identifier(_)));

        if has_catch_all {
            return; // Exhaustivo
        }

        match scrutinee_type {
            Type::Enum { name } => {
                let Some(enum_info) = self.enum_infos.get(name) else {
                    return;
                };

                let mut covered_variants: HashSet<String> = HashSet::new();
                let mut payload_coverage: HashMap<String, bool> = enum_info
                    .variants
                    .iter()
                    .filter(|(_, info)| info.data.as_ref().map(|d| !d.is_empty()).unwrap_or(false))
                    .map(|(variant, _)| (variant.clone(), false))
                    .collect();

                for arm in arms {
                    if let Pattern::EnumVariant {
                        enum_name,
                        variant_name,
                        data,
                        ..
                    } = &arm.pattern
                    {
                        if enum_name != name {
                            continue;
                        }

                        covered_variants.insert(variant_name.clone());

                        if let Some(flag) = payload_coverage.get_mut(variant_name) {
                            let expected_len = enum_info
                                .variants
                                .get(variant_name)
                                .and_then(|info| info.data.as_ref())
                                .map(|data| data.len())
                                .unwrap_or(0);

                            if let Some(payload_patterns) = data {
                                if payload_patterns.len() == expected_len
                                    && payload_patterns.iter().all(|p| {
                                        matches!(p, Pattern::Wildcard | Pattern::Identifier(_))
                                    })
                                {
                                    *flag = true;
                                }
                            }
                        }
                    }
                }

                let missing_variants: Vec<String> = enum_info
                    .variants
                    .keys()
                    .filter(|variant| !covered_variants.contains(*variant))
                    .cloned()
                    .collect();

                if !missing_variants.is_empty() {
                    let missing_str = missing_variants
                        .into_iter()
                        .map(|variant| format!("{}::{}", name, variant))
                        .collect::<Vec<_>>()
                        .join(", ");

                    self.error(
                        format!(
                            "Match expression is not exhaustive. Missing patterns: {}",
                            missing_str
                        ),
                        span,
                    );
                    return;
                }

                let missing_payload_guard: Vec<String> = payload_coverage
                    .into_iter()
                    .filter_map(|(variant, covered)| {
                        if covered {
                            None
                        } else {
                            Some(format!("{}::{}", name, variant))
                        }
                    })
                    .collect();

                if !missing_payload_guard.is_empty() {
                    let list = missing_payload_guard.join(", ");
                    self.error(
                        format!(
                            "Match on enum '{}' must include wildcard bindings for payload of variant(s): {}",
                            name, list
                        ),
                        span,
                    );
                }
            }
            Type::Bool => {
                let has_true = arms.iter().any(|arm| {
                    if let Pattern::Literal(expr) = &arm.pattern {
                        matches!(expr.kind, ExpressionKind::BoolLiteral(true))
                    } else {
                        false
                    }
                });
                let has_false = arms.iter().any(|arm| {
                    if let Pattern::Literal(expr) = &arm.pattern {
                        matches!(expr.kind, ExpressionKind::BoolLiteral(false))
                    } else {
                        false
                    }
                });

                if !(has_true && has_false) {
                    self.error(
                        "Match on 'bool' is not exhaustive. Consider adding 'true', 'false', or a wildcard pattern (_).",
                        span,
                    );
                }
            }
            Type::Tuple { elements } => {
                if elements.is_empty() {
                    return;
                }

                let mut bool_combinations: HashSet<Vec<bool>> = HashSet::new();
                let mut unsupported_pattern = false;

                for arm in arms {
                    if let Pattern::Literal(expr) = &arm.pattern {
                        if let ExpressionKind::TupleLiteral {
                            elements: tuple_elems,
                        } = &expr.kind
                        {
                            if tuple_elems.len() != elements.len() {
                                unsupported_pattern = true;
                                break;
                            }

                            let mut combo = Vec::with_capacity(elements.len());
                            let mut tuple_supported = true;

                            for (tuple_ty, tuple_expr) in elements.iter().zip(tuple_elems.iter()) {
                                match (tuple_ty, &tuple_expr.kind) {
                                    (Type::Bool, ExpressionKind::BoolLiteral(value)) => {
                                        combo.push(*value);
                                    }
                                    _ => {
                                        tuple_supported = false;
                                        break;
                                    }
                                }
                            }

                            if tuple_supported {
                                bool_combinations.insert(combo);
                            } else {
                                unsupported_pattern = true;
                                break;
                            }
                        } else {
                            unsupported_pattern = true;
                            break;
                        }
                    } else {
                        unsupported_pattern = true;
                        break;
                    }
                }

                if unsupported_pattern {
                    self.error(
                        "Match on tuple requires a wildcard (_) pattern to cover remaining combinations.",
                        span,
                    );
                    return;
                }

                let expected = 1 << elements.len();
                if bool_combinations.len() != expected {
                    self.error(
                        format!(
                            "Match on tuple of bools is not exhaustive. Expected {} combination(s).",
                            expected
                        ),
                        span,
                    );
                }
            }
            _ => {
                let only_literals = arms
                    .iter()
                    .all(|arm| matches!(arm.pattern, Pattern::Literal(_)));

                if only_literals {
                    self.error(
                        "Match expression with only literal patterns is not exhaustive. Consider adding a wildcard pattern (_).",
                        span,
                    );
                }
            }
        }
    }

    // Third pass: fill type information in method calls
    fn fill_method_call_types_in_item(&mut self, item: &mut Item) {
        match item {
            Item::Function(func) => {
                self.fill_method_call_types_in_block(&mut func.body);
            }
            Item::Impl(impl_block) => {
                for method in &mut impl_block.methods {
                    self.fill_method_call_types_in_block(&mut method.body);
                }
            }
            Item::TraitImpl(trait_impl) => {
                for method in &mut trait_impl.methods {
                    self.fill_method_call_types_in_block(&mut method.body);
                }
            }
            _ => {}
        }
    }

    fn fill_method_call_types_in_block(&mut self, block: &mut crate::ast::Block) {
        for stmt in &mut block.statements {
            self.fill_method_call_types_in_statement(stmt);
        }
    }

    fn fill_method_call_types_in_statement(&mut self, stmt: &mut Statement) {
        use crate::ast::StatementKind;

        match &mut stmt.kind {
            StatementKind::Let(let_stmt) => {
                if let Some(value) = &mut let_stmt.value {
                    self.fill_method_call_types_in_expression(value);
                }
            }
            StatementKind::Assignment(assign) => {
                self.fill_method_call_types_in_expression(&mut assign.value);
            }
            StatementKind::While(while_loop) => {
                self.fill_method_call_types_in_expression(&mut while_loop.condition);
                self.fill_method_call_types_in_block(&mut while_loop.body);
            }
            StatementKind::DoWhile(do_while) => {
                self.fill_method_call_types_in_block(&mut do_while.body);
                self.fill_method_call_types_in_expression(&mut do_while.condition);
            }
            StatementKind::For(for_loop) => {
                self.fill_method_call_types_in_expression(&mut for_loop.iterable);
                self.fill_method_call_types_in_block(&mut for_loop.body);
            }
            StatementKind::Loop(loop_stmt) => {
                self.fill_method_call_types_in_block(&mut loop_stmt.body);
            }
            StatementKind::Expression(expr) => {
                self.fill_method_call_types_in_expression(expr);
            }
            StatementKind::Return(ret_stmt) => {
                if let Some(expr) = &mut ret_stmt.value {
                    self.fill_method_call_types_in_expression(expr);
                }
            }
            _ => {}
        }
    }

    fn fill_method_call_types_in_expression(&mut self, expr: &mut Expression) {
        use crate::ast::ExpressionKind;

        match &mut expr.kind {
            ExpressionKind::MethodCall {
                object,
                method_name: _,
                arguments,
                type_name,
            } => {
                // Primeiro, processar recursivamente o objeto e argumentos
                self.fill_method_call_types_in_expression(object);
                for arg in arguments {
                    self.fill_method_call_types_in_expression(arg);
                }

                // Se type_name ainda não foi preenchido, inferir agora
                if type_name.is_none() {
                    let obj_type = self.infer_expression_type(object);
                    *type_name = match obj_type {
                        Type::Struct { name } => Some(name),
                        Type::Enum { name, .. } => Some(name),
                        _ => None,
                    };
                }
            }
            ExpressionKind::Call { callee, arguments } => {
                self.fill_method_call_types_in_expression(callee);
                for arg in arguments {
                    self.fill_method_call_types_in_expression(arg);
                }
            }
            ExpressionKind::Binary { left, right, .. } => {
                self.fill_method_call_types_in_expression(left);
                self.fill_method_call_types_in_expression(right);
            }
            ExpressionKind::Unary { operand, .. } => {
                self.fill_method_call_types_in_expression(operand);
            }
            ExpressionKind::FieldAccess { object, .. } => {
                self.fill_method_call_types_in_expression(object);
            }
            ExpressionKind::IndexAccess { array, index } => {
                self.fill_method_call_types_in_expression(array);
                self.fill_method_call_types_in_expression(index);
            }
            ExpressionKind::TupleAccess { tuple, .. } => {
                self.fill_method_call_types_in_expression(tuple);
            }
            ExpressionKind::Match { scrutinee, arms } => {
                self.fill_method_call_types_in_expression(scrutinee);
                for arm in arms {
                    self.fill_method_call_types_in_expression(&mut arm.body);
                }
            }
            _ => {}
        }
    }

    // ============= Type Inference Pass =============

    fn infer_generic_types_in_item(&mut self, item: &mut Item) {
        match item {
            Item::Function(func) => {
                self.infer_generic_types_in_block(&mut func.body);
            }
            Item::Impl(impl_block) => {
                for method in &mut impl_block.methods {
                    self.infer_generic_types_in_block(&mut method.body);
                }
            }
            Item::TraitImpl(trait_impl) => {
                for method in &mut trait_impl.methods {
                    self.infer_generic_types_in_block(&mut method.body);
                }
            }
            _ => {}
        }
    }

    fn infer_generic_types_in_block(&mut self, block: &mut Block) {
        for stmt in &mut block.statements {
            self.infer_generic_types_in_statement(stmt);
        }
    }

    fn infer_generic_types_in_statement(&mut self, stmt: &mut Statement) {
        use crate::ast::StatementKind;

        match &mut stmt.kind {
            StatementKind::Let(let_stmt) => {
                if let Some(value) = &mut let_stmt.value {
                    self.infer_generic_types_in_expression(value);
                }
            }
            StatementKind::Assignment(assign) => {
                self.infer_generic_types_in_expression(&mut assign.value);
            }
            StatementKind::While(while_loop) => {
                self.infer_generic_types_in_expression(&mut while_loop.condition);
                self.infer_generic_types_in_block(&mut while_loop.body);
            }
            StatementKind::DoWhile(do_while_loop) => {
                self.infer_generic_types_in_block(&mut do_while_loop.body);
                self.infer_generic_types_in_expression(&mut do_while_loop.condition);
            }
            StatementKind::For(for_loop) => {
                self.infer_generic_types_in_expression(&mut for_loop.iterable);
                self.infer_generic_types_in_block(&mut for_loop.body);
            }
            StatementKind::Expression(expr) => {
                self.infer_generic_types_in_expression(expr);
            }
            StatementKind::Return(ret_stmt) => {
                if let Some(value) = &mut ret_stmt.value {
                    self.infer_generic_types_in_expression(value);
                }
            }
            _ => {}
        }
    }

    fn infer_generic_types_in_expression(&mut self, expr: &mut Expression) {
        match &mut expr.kind {
            ExpressionKind::StructLiteral {
                name,
                type_args,
                fields,
            } => {
                // Infer type arguments if not provided and struct is generic
                if type_args.is_empty() {
                    if let Some((type_params, field_defs)) = self.generic_structs.get(name).cloned()
                    {
                        // Attempt to infer type arguments from field values
                        let inferred_types =
                            self.infer_struct_type_args(&type_params, &field_defs, fields);
                        if !inferred_types.is_empty() {
                            // Update the expression with inferred type arguments
                            *type_args = inferred_types;
                        }
                    }
                }

                // Recurse into field values
                for (_, field_value) in fields {
                    self.infer_generic_types_in_expression(field_value);
                }
            }
            ExpressionKind::EnumVariant {
                enum_name,
                type_args,
                variant_name,
                data,
            } => {
                if let Some(args) = data {
                    for arg in args.iter_mut() {
                        self.infer_generic_types_in_expression(arg);
                    }

                    if type_args.is_empty() {
                        let inferred_args =
                            self.infer_enum_type_args(enum_name, variant_name, args.as_slice());
                        if !inferred_args.is_empty() {
                            *type_args = inferred_args;
                        }
                    }
                }
            }
            ExpressionKind::Binary { left, right, .. } => {
                self.infer_generic_types_in_expression(left);
                self.infer_generic_types_in_expression(right);
            }
            ExpressionKind::Unary { operand, .. } => {
                self.infer_generic_types_in_expression(operand);
            }
            ExpressionKind::Call { arguments, .. } => {
                for arg in arguments {
                    self.infer_generic_types_in_expression(arg);
                }
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.infer_generic_types_in_expression(condition);
                self.infer_generic_types_in_block(then_block);
                for (elif_cond, elif_block) in elif_blocks {
                    self.infer_generic_types_in_expression(elif_cond);
                    self.infer_generic_types_in_block(elif_block);
                }
                if let Some(else_body) = else_block {
                    self.infer_generic_types_in_block(else_body);
                }
            }
            ExpressionKind::Match { scrutinee, arms } => {
                self.infer_generic_types_in_expression(scrutinee);
                for arm in arms {
                    self.infer_generic_types_in_expression(&mut arm.body);
                }
            }
            ExpressionKind::ArrayLiteral { elements } => {
                for elem in elements {
                    self.infer_generic_types_in_expression(elem);
                }
            }
            ExpressionKind::FieldAccess { object, .. } => {
                self.infer_generic_types_in_expression(object);
            }
            ExpressionKind::IndexAccess { array, index } => {
                self.infer_generic_types_in_expression(array);
                self.infer_generic_types_in_expression(index);
            }
            _ => {}
        }
    }

    /// Infer type arguments for a generic struct from field values
    fn infer_struct_type_args(
        &mut self,
        type_params: &[crate::ast::TypeParameter],
        field_defs: &[(String, crate::ast::TypeAnnotation)],
        field_values: &[(String, Expression)],
    ) -> Vec<crate::ast::TypeAnnotation> {
        // Create a map to store inferred types for each type parameter
        let mut type_map: HashMap<String, Type> = HashMap::new();

        // For each field value, infer its type and match against field definition
        for (field_name, field_expr) in field_values {
            // Find the field definition
            if let Some((_, field_type_ann)) =
                field_defs.iter().find(|(name, _)| name == field_name)
            {
                // Infer the type of the field expression
                let value_type = self.infer_expression_type(field_expr);

                // Try to unify the field type annotation with the value type
                self.unify_type_annotation(field_type_ann, &value_type, &mut type_map);
            }
        }

        // Convert inferred types to TypeAnnotation in the order of type_params
        let mut result = Vec::new();
        for param in type_params {
            if let Some(inferred_type) = type_map.get(&param.name) {
                let type_ann = self.type_to_annotation(inferred_type);
                result.push(type_ann);
            } else {
                // Could not infer this type parameter, return empty to indicate failure
                return Vec::new();
            }
        }

        result
    }

    /// Infer type arguments for a generic enum based on a variant constructor call
    fn infer_enum_type_args(
        &mut self,
        enum_name: &str,
        variant_name: &str,
        arg_exprs: &[Expression],
    ) -> Vec<crate::ast::TypeAnnotation> {
        let (type_params, _) = match self.generic_enums.get(enum_name) {
            Some(info) => info.clone(),
            None => return Vec::new(),
        };

        let variant_info = match self
            .enum_infos
            .get(enum_name)
            .and_then(|info| info.variants.get(variant_name))
            .cloned()
        {
            Some(info) => info,
            None => return Vec::new(),
        };

        let field_type_annotations = match variant_info.data {
            Some(data) if data.len() == arg_exprs.len() && !data.is_empty() => data,
            _ => return Vec::new(),
        };

        let mut type_map: HashMap<String, Type> = HashMap::new();
        for (field_ann, arg_expr) in field_type_annotations.iter().zip(arg_exprs) {
            let value_type = self.infer_expression_type(arg_expr);
            self.unify_type_annotation(field_ann, &value_type, &mut type_map);
        }

        let mut result = Vec::new();
        for param in type_params {
            match type_map.get(&param.name) {
                Some(mapped) if !matches!(mapped, Type::Unknown) => {
                    result.push(self.type_to_annotation(mapped));
                }
                _ => return Vec::new(),
            }
        }

        result
    }

    /// Unify a type annotation (potentially containing type variables) with a concrete type
    fn unify_type_annotation(
        &self,
        type_ann: &crate::ast::TypeAnnotation,
        concrete_type: &Type,
        type_map: &mut HashMap<String, Type>,
    ) {
        use crate::ast::TypeAnnotationKind;

        match &type_ann.kind {
            TypeAnnotationKind::Simple { segments } => {
                // If it's a single segment, it might be a type parameter
                if segments.len() == 1 {
                    let name = &segments[0];
                    // Check if this could be a type parameter (starts with uppercase typically)
                    // For now, we'll assume any single segment could be a type parameter
                    // and try to map it
                    match type_map.entry(name.clone()) {
                        Entry::Occupied(mut entry) => {
                            if !self.types_compatible(entry.get(), concrete_type) {
                                entry.insert(Type::Unknown);
                            }
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(concrete_type.clone());
                        }
                    }
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                if let Type::Tuple {
                    elements: concrete_elements,
                } = concrete_type
                {
                    if elements.len() != concrete_elements.len() {
                        return;
                    }
                    // Unify each element
                    for (elem_ann, elem_type) in elements.iter().zip(concrete_elements.iter()) {
                        self.unify_type_annotation(elem_ann, elem_type, type_map);
                    }
                }
            }
        }
    }

    /// Convert a Type to TypeAnnotation
    fn type_to_annotation(&self, ty: &Type) -> crate::ast::TypeAnnotation {
        use crate::ast::{TypeAnnotation, TypeAnnotationKind};
        use crate::span::Span;

        let kind = match ty {
            Type::Int => TypeAnnotationKind::Simple {
                segments: vec!["int".to_string()],
            },
            Type::Float => TypeAnnotationKind::Simple {
                segments: vec!["float".to_string()],
            },
            Type::Bool => TypeAnnotationKind::Simple {
                segments: vec!["bool".to_string()],
            },
            Type::String => TypeAnnotationKind::Simple {
                segments: vec!["string".to_string()],
            },
            Type::Char => TypeAnnotationKind::Simple {
                segments: vec!["char".to_string()],
            },
            Type::Unit => TypeAnnotationKind::Simple {
                segments: vec!["void".to_string()],
            },
            Type::Struct { name } => TypeAnnotationKind::Simple {
                segments: vec![name.clone()],
            },
            Type::Enum { name, .. } => TypeAnnotationKind::Simple {
                segments: vec![name.clone()],
            },
            Type::Tuple { elements } => {
                let element_anns = elements
                    .iter()
                    .map(|el| self.type_to_annotation(el))
                    .collect();
                TypeAnnotationKind::Tuple {
                    elements: element_anns,
                }
            }
            Type::Array { element_type, .. } => {
                // For arrays, we'll just use the element type name
                let elem_ann = self.type_to_annotation(element_type);
                // Simplification: return element type (proper array annotation would need size)
                elem_ann.kind
            }
            Type::TypeParameter { name } => TypeAnnotationKind::Simple {
                segments: vec![name.clone()],
            },
            Type::SelfType => TypeAnnotationKind::Simple {
                segments: vec!["Self".to_string()],
            },
            Type::Unknown => TypeAnnotationKind::Simple {
                segments: vec!["unknown".to_string()],
            },
        };

        TypeAnnotation {
            kind,
            span: Span {
                start: 0,
                end: 0,
                start_location: crate::span::Location { line: 0, column: 0 },
                end_location: crate::span::Location { line: 0, column: 0 },
            },
        }
    }
}
