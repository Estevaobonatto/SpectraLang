use crate::{
    ast::{
        Block, Expression, ExpressionKind, Function, Item, Module, Pattern, Statement,
        StatementKind, Type,
    },
    error::SemanticError,
    span::Span,
};
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<Type>,
    return_type: Type,
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
    // Trait implementations: maps (trait_name, type_name) to validation status
    trait_impls: HashMap<(String, String), bool>,
    // Struct metadata for validation and lookup
    struct_infos: HashMap<String, StructInfo>,
    // Enum metadata (including variant payload types)
    enum_infos: HashMap<String, EnumInfo>,
    // Generic structs: maps struct_name to (type_params, field_definitions)
    generic_structs: HashMap<String, (Vec<String>, Vec<(String, crate::ast::TypeAnnotation)>)>,
    // Generic enums: maps enum_name to (type_params, variants)
    generic_enums: HashMap<String, (Vec<String>, Vec<String>)>,
    // Track if we're inside a loop (for break/continue validation)
    loop_depth: usize,
    // Track if we're inside a function (for return validation)
    current_function: Option<String>,
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
                        _ => Type::Unknown,
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

    fn types_match(&self, actual: &Type, expected: &Type) -> bool {
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
                        // Extract parameter types
                        let params: Vec<Type> = func
                            .params
                            .iter()
                            .map(|p| self.type_annotation_to_type(&p.ty))
                            .collect();

                        // Extract return type
                        let return_type = self.type_annotation_to_type(&func.return_type);

                        let signature = FunctionSignature {
                            params,
                            return_type: return_type.clone(),
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
                        let type_param_names: Vec<String> = struct_def
                            .type_params
                            .iter()
                            .map(|tp| tp.name.clone())
                            .collect();
                        let fields: Vec<(String, crate::ast::TypeAnnotation)> = struct_def
                            .fields
                            .iter()
                            .map(|f| (f.name.clone(), f.ty.clone()))
                            .collect();
                        self.generic_structs
                            .insert(struct_def.name.clone(), (type_param_names, fields));
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
                        let type_param_names: Vec<String> = enum_def
                            .type_params
                            .iter()
                            .map(|tp| tp.name.clone())
                            .collect();
                        let variant_names: Vec<String> =
                            enum_def.variants.iter().map(|v| v.name.clone()).collect();
                        self.generic_enums
                            .insert(enum_def.name.clone(), (type_param_names, variant_names));
                    }
                }
                _ => {}
            }
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
            Item::TraitImpl(_trait_impl) => {
                // TraitImpl não é mais usado - impl Trait for Type
                // é parseado como ImplBlock regular
            }
        }
    }

    fn analyze_impl_block(&mut self, impl_block: &crate::ast::ImplBlock) {
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
            for param in &method.params {
                if param.is_self {
                    // self parameter - tipo é o do impl block
                    // TODO: Distinguir entre self, &self, &mut self
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

            self.pop_scope();
            self.current_function = None;
        }
    }

    /// Analisa declaração de trait e registra assinaturas dos métodos
    fn analyze_trait_declaration(&mut self, trait_decl: &crate::ast::TraitDeclaration) {
        let mut trait_methods = HashMap::new();

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
        }

        // Then add this trait's own methods (can override inherited methods)
        for method in &trait_decl.methods {
            // Converter parâmetros para Type
            let mut param_types = Vec::new();
            for param in &method.params {
                if param.is_self {
                    // self em trait é genérico - será o tipo que implementa o trait
                    param_types.push(Type::Unknown);
                } else {
                    let param_type = self.type_annotation_to_type(&param.type_annotation);
                    param_types.push(param_type);
                }
            }

            let return_type = self.type_annotation_to_type(&method.return_type);

            let signature = FunctionSignature {
                params: param_types,
                return_type,
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

        // Coletar métodos implementados
        let mut implemented_methods = HashMap::new();
        for method in &impl_block.methods {
            // Converter parâmetros para Type
            let mut param_types = Vec::new();
            for param in &method.params {
                if param.is_self {
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
            };

            implemented_methods.insert(method.name.clone(), (signature, method.span));
        }

        // Verificar que todos os métodos do trait foram implementados
        for (trait_method_name, trait_method_info) in &trait_methods {
            match implemented_methods.get(trait_method_name) {
                Some((impl_signature, _span)) => {
                    // Verificar que as assinaturas correspondem
                    // Primeiro parâmetro do trait é Unknown (self genérico), então pulamos
                    // Mas apenas se houver parâmetros (métodos estáticos não têm self)
                    let trait_params = if !trait_method_info.signature.params.is_empty() {
                        &trait_method_info.signature.params[1..] // Pula self
                    } else {
                        &trait_method_info.signature.params[..] // Sem parâmetros
                    };

                    let impl_params = if !impl_signature.params.is_empty() {
                        &impl_signature.params[1..] // Pula self
                    } else {
                        &impl_signature.params[..] // Sem parâmetros
                    };

                    if trait_params.len() != impl_params.len() {
                        self.error(
                            format!(
                                "Method '{}' has wrong number of parameters. Expected {}, found {}",
                                trait_method_name,
                                trait_params.len(),
                                impl_params.len()
                            ),
                            impl_block.span,
                        );
                        continue;
                    }

                    // Verificar tipos dos parâmetros
                    for (i, (trait_param, impl_param)) in
                        trait_params.iter().zip(impl_params.iter()).enumerate()
                    {
                        if !self.types_match(impl_param, trait_param) {
                            self.error(
                                format!(
                                    "Method '{}' parameter {} has wrong type. Expected {:?}, found {:?}",
                                    trait_method_name,
                                    i + 1,
                                    trait_param,
                                    impl_param
                                ),
                                impl_block.span,
                            );
                        }
                    }

                    // Verificar tipo de retorno
                    if !self.types_match(
                        &impl_signature.return_type,
                        &trait_method_info.signature.return_type,
                    ) {
                        self.error(
                            format!(
                                "Method '{}' has wrong return type. Expected {:?}, found {:?}",
                                trait_method_name,
                                trait_method_info.signature.return_type,
                                impl_signature.return_type
                            ),
                            impl_block.span,
                        );
                    }
                }
                None => {
                    // Método não implementado - OK se tem default, erro caso contrário
                    if !trait_method_info.has_default {
                        self.error(
                            format!(
                                "Type '{}' does not implement required trait method '{}' (no default implementation)",
                                impl_block.type_name, trait_method_name
                            ),
                            impl_block.span,
                        );
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
                    if i == 0 {
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
        self.current_function = Some(func.name.clone());
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

        self.pop_scope();
        self.current_function = None;
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
                    self.error(
                        format!(
                            "Cannot assign value of type {:?} to target of type {:?}",
                            value_type, target_type
                        ),
                        assign_stmt.value.span,
                    );
                }
            }
            StatementKind::Return(ret_stmt) => {
                if self.current_function.is_none() {
                    self.error("Return statement outside of function", ret_stmt.span);
                }

                if let Some(ref value) = ret_stmt.value {
                    self.analyze_expression(value);
                }
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
                        // If either operand is string, result is string (concatenation)
                        if matches!(left_type, Type::String) || matches!(right_type, Type::String) {
                            Type::String
                        } else {
                            left_type
                        }
                    }
                    BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => left_type,
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
            ExpressionKind::Unary {
                operator: _,
                operand,
            } => self.infer_expression_type(operand),
            ExpressionKind::Call {
                callee,
                arguments: _,
            } => {
                if let ExpressionKind::Identifier(name) = &callee.kind {
                    if let Some(sig) = self.functions.get(name) {
                        return sig.return_type.clone();
                    }
                }
                Type::Unknown
            }
            ExpressionKind::If { .. } => Type::Unknown, // TODO: inferir tipo comum dos ramos
            ExpressionKind::Unless { .. } => Type::Unknown, // TODO: inferir tipo comum dos ramos
            ExpressionKind::Grouping(inner) => self.infer_expression_type(inner),
            ExpressionKind::ArrayLiteral { elements } => {
                if elements.is_empty() {
                    // Array vazio, tipo desconhecido
                    Type::Array {
                        element_type: Box::new(Type::Unknown),
                        size: Some(0),
                    }
                } else {
                    // Inferir tipo do primeiro elemento
                    let elem_type = self.infer_expression_type(&elements[0]);
                    Type::Array {
                        element_type: Box::new(elem_type),
                        size: Some(elements.len()),
                    }
                }
            }
            ExpressionKind::IndexAccess { array, index: _ } => {
                let array_type = self.infer_expression_type(array);
                match array_type {
                    Type::Array { element_type, .. } => *element_type,
                    _ => Type::Unknown,
                }
            }
            ExpressionKind::TupleLiteral { elements } => {
                if elements.is_empty() {
                    // Empty tuple - unit type
                    Type::Tuple { elements: vec![] }
                } else {
                    // Infer type of each element
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
            ExpressionKind::FieldAccess {
                object,
                field,
            } => {
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
                // TODO: Verificar exhaustividade do match
                // Por enquanto, retornar tipo do primeiro arm
                if let Some(first_arm) = arms.first() {
                    self.infer_expression_type(&first_arm.body)
                } else {
                    Type::Unknown
                }
            }
            ExpressionKind::MethodCall {
                object,
                method_name,
                arguments: _,
                type_name: _,
            } => {
                // Inferir tipo de retorno do método baseado na assinatura
                let obj_type = self.infer_expression_type(object);

                // Extrair nome do tipo
                let type_name = match &obj_type {
                    Type::Struct { name } => Some(name.clone()),
                    Type::Enum { name, .. } => Some(name.clone()),
                    _ => None,
                };

                // Buscar assinatura do método
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
                        // Check if types match
                        if left_type != Type::Unknown
                            && right_type != Type::Unknown
                            && left_type != right_type
                        {
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
                        self.error(
                            format!("Struct '{}' is not defined", name),
                            expr.span,
                        );
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
                            self.error(
                                format!(
                                    "Field '{}' in struct '{}' has type {:?}, but {:?} was expected",
                                    field_name, name, value_type, expected_type
                                ),
                                field_value.span,
                            );
                        }
                    } else {
                        self.error(
                            format!(
                                "Struct '{}' has no field named '{}'",
                                name, field_name
                            ),
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
                                    format!(
                                        "Struct '{}' has no field named '{}'",
                                        name, field
                                    ),
                                    expr.span,
                                );
                            }
                        } else {
                            self.error(
                                format!("Struct '{}' is not defined", name),
                                expr.span,
                            );
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
                        self.error(
                            format!("Enum '{}' is not defined", enum_name),
                            expr.span,
                        );
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

                        for (idx, (expected_ann, arg_expr)) in expected_params
                            .iter()
                            .zip(actual_args.iter())
                            .enumerate()
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

                // Verificar exhaustiveness
                self.check_match_exhaustiveness(arms, expr.span);

                // TODO: Verificar tipos consistentes nos arms
                for arm in arms {
                    // Criar novo escopo para o arm
                    self.push_scope();

                    // Registrar variáveis do pattern
                    self.register_pattern_bindings(&arm.pattern);

                    // Analisar corpo do arm
                    self.analyze_expression(&arm.body);

                    // Sair do escopo
                    self.pop_scope();
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
                    // Clonar a assinatura para evitar problemas de borrow
                    let method_signature = self
                        .methods
                        .get(type_name)
                        .and_then(|methods| methods.get(method_name).cloned());

                    // Se não encontrou o método explicitamente, buscar em traits com implementação padrão
                    let signature = if let Some(sig) = method_signature {
                        Some(sig)
                    } else {
                        // Procurar em todos os traits implementados por este tipo
                        let mut found_signature = None;
                        for ((trait_name, impl_type), _) in &self.trait_impls {
                            if impl_type == type_name {
                                if let Some(trait_methods) = self.traits.get(trait_name) {
                                    if let Some(trait_method_info) = trait_methods.get(method_name)
                                    {
                                        if trait_method_info.has_default {
                                            found_signature =
                                                Some(trait_method_info.signature.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        found_signature
                    };

                    if let Some(signature) = signature {
                        // Validar número de argumentos
                        // Nota: signature.params inclui self como primeiro parâmetro
                        let expected_args = if signature.params.is_empty() {
                            0
                        } else {
                            signature.params.len() - 1 // Subtrair self
                        };

                        if arguments.len() != expected_args {
                            self.error(
                                format!(
                                    "Method '{}' expects {} argument(s), but {} were provided",
                                    method_name,
                                    expected_args,
                                    arguments.len()
                                ),
                                expr.span,
                            );
                        }

                        // Validar tipos dos argumentos
                        for (i, arg) in arguments.iter().enumerate() {
                            let arg_type = self.infer_expression_type(arg);
                            // +1 porque params[0] é self
                            if let Some(expected_type) = signature.params.get(i + 1) {
                                if !self.types_match(&arg_type, expected_type) {
                                    self.error(
                                        format!(
                                            "Method '{}' argument {} has type {:?}, but {:?} was expected",
                                            method_name,
                                            i + 1,
                                            arg_type,
                                            expected_type
                                        ),
                                        arg.span,
                                    );
                                }
                            }
                        }
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

    /// Verifica se um match expression é exhaustivo
    fn check_match_exhaustiveness(&mut self, arms: &[crate::ast::MatchArm], span: Span) {
        use crate::ast::Pattern;

        // Se tem wildcard ou identifier, é automaticamente exhaustivo
        let has_catch_all = arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard | Pattern::Identifier(_)));

        if has_catch_all {
            return; // Exhaustivo
        }

        // Coletar todos os enum variants cobertos
        let mut covered_variants: HashMap<String, Vec<String>> = HashMap::new();

        for arm in arms {
            if let Pattern::EnumVariant {
                enum_name,
                variant_name,
                ..
            } = &arm.pattern
            {
                covered_variants
                    .entry(enum_name.clone())
                    .or_insert_with(Vec::new)
                    .push(variant_name.clone());
            }
        }

        // Verificar se todos os variants de cada enum estão cobertos
        for (enum_name, covered) in &covered_variants {
            if let Some(all_variants) = self.enum_definitions.get(enum_name) {
                let missing: Vec<&String> = all_variants
                    .iter()
                    .filter(|v| !covered.contains(v))
                    .collect();

                if !missing.is_empty() {
                    let missing_str = missing
                        .iter()
                        .map(|v| format!("{}::{}", enum_name, v))
                        .collect::<Vec<_>>()
                        .join(", ");

                    self.error(
                        format!(
                            "Match expression is not exhaustive. Missing patterns: {}",
                            missing_str
                        ),
                        span,
                    );
                }
            }
        }

        // Se tem apenas literais, verificar se cobre todos os casos de bool
        let only_literals = arms
            .iter()
            .all(|arm| matches!(arm.pattern, Pattern::Literal(_)));

        if only_literals {
            // Verificar se tem true E false (exhaustivo para bool)
            use crate::ast::ExpressionKind;
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

            // Se tem true E false, é exhaustivo para bool
            if has_true && has_false {
                return;
            }

            self.error(
                "Match expression with only literal patterns is not exhaustive. Consider adding a wildcard pattern (_).",
                span,
            );
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
                enum_name: _,
                type_args: _,
                variant_name: _,
                data,
            } => {
                // TODO: Implement enum type inference
                // For now, just recurse into data
                if let Some(args) = data {
                    for arg in args {
                        self.infer_generic_types_in_expression(arg);
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
        type_params: &[String],
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
            if let Some(inferred_type) = type_map.get(param) {
                let type_ann = self.type_to_annotation(inferred_type);
                result.push(type_ann);
            } else {
                // Could not infer this type parameter, return empty to indicate failure
                return Vec::new();
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
                    if !type_map.contains_key(name) {
                        type_map.insert(name.clone(), concrete_type.clone());
                    }
                    // TODO: Check consistency if already mapped
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                if let Type::Tuple {
                    elements: concrete_elements,
                } = concrete_type
                {
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
