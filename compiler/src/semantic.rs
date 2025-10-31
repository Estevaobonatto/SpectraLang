use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::convert::TryFrom;

use crate::ast::{
    BinaryOperator, Block, Constant, EnumLiteralKind, EnumPatternKind, Export, Expr, Function,
    Import, Item, Literal, MatchPattern, Module, ModulePath, Stmt, TypeName, UnaryOperator,
    Visibility,
};
use crate::error::{SemanticError, SemanticResult};
use crate::span::Span;

#[derive(Clone)]
struct FunctionExport {
    signature: FunctionType,
    span: Span,
}

#[derive(Clone)]
struct ConstantExport {
    ty: Type,
    span: Span,
}

#[derive(Clone)]
struct StructExport {
    ty: Type,
    span: Span,
}

#[derive(Clone)]
enum EnumVariantInfo {
    Unit,
    Tuple(Vec<Type>),
    Struct(HashMap<String, Type>),
}

#[derive(Clone)]
struct EnumExport {
    ty: Type,
    variants: HashMap<String, EnumVariantInfo>,
    span: Span,
}

#[derive(Clone, Default)]
struct EnumInfo {
    variants: HashMap<String, EnumVariantInfo>,
}

#[derive(Clone, Default)]
struct ModuleExport {
    functions: HashMap<String, FunctionExport>,
    constants: HashMap<String, ConstantExport>,
    structs: HashMap<String, StructExport>,
    enums: HashMap<String, EnumExport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModuleAliasInfo {
    alias: String,
    module_key: String,
    span: Span,
}

pub fn analyze(module: &Module) -> SemanticResult<()> {
    analyze_modules(&[module])
}

pub fn analyze_modules(modules: &[&Module]) -> SemanticResult<()> {
    let mut analyzer = Analyzer::new();
    analyzer.register_modules(modules);
    analyzer.collect_exports(modules);
    for module in modules {
        analyzer.analyze_module(module);
    }
    if analyzer.errors.is_empty() {
        Ok(())
    } else {
        Err(analyzer.errors)
    }
}

struct Analyzer<'a> {
    errors: Vec<SemanticError>,
    scopes: ScopeStack,
    current_function: Option<FunctionContext<'a>>,
    module_registry: HashMap<String, Span>,
    module_exports: HashMap<String, ModuleExport>,
    current_module_key: Option<String>,
    current_module_signatures: HashMap<String, FunctionType>,
    struct_registry: HashMap<String, Type>,
    enum_registry: HashMap<String, EnumInfo>,
    current_module_imports: HashSet<String>,
    module_aliases: HashMap<String, ModuleAliasInfo>,
    loop_depth: usize,
}

impl<'a> Analyzer<'a> {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            scopes: ScopeStack::new(),
            current_function: None,
            module_registry: HashMap::new(),
            module_exports: HashMap::new(),
            current_module_key: None,
            current_module_signatures: HashMap::new(),
            struct_registry: HashMap::new(),
            enum_registry: HashMap::new(),
            current_module_imports: HashSet::new(),
            module_aliases: HashMap::new(),
            loop_depth: 0,
        }
    }

    fn register_modules(&mut self, modules: &[&'a Module]) {
        for module in modules {
            if let Some(path) = module.name.as_ref() {
                let key = module_path_key(path);
                match self.module_registry.entry(key.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(path.span);
                    }
                    Entry::Occupied(entry) => {
                        self.errors.push(SemanticError::new(
                            format!(
                                "module '{}' already defined (previous definition at {})",
                                key,
                                entry.get().start_location
                            ),
                            path.span,
                        ));
                    }
                }
            }
        }
    }

    fn collect_exports(&mut self, modules: &[&'a Module]) {
        for module in modules {
            let Some(path) = module.name.as_ref() else {
                continue;
            };
            let key = module_path_key(path);
            for item in &module.items {
                match item {
                    Item::Function(function) => {
                        if function.visibility != Visibility::Public {
                            continue;
                        }
                        let placeholder_signature = FunctionType {
                            parameters: vec![Type::Unknown; function.parameters.len()],
                            return_type: Box::new(if function.is_async {
                                Type::Future(Box::new(if function.return_type.is_some() {
                                    Type::Unknown
                                } else {
                                    Type::Void
                                }))
                            } else if function.return_type.is_some() {
                                Type::Unknown
                            } else {
                                Type::Void
                            }),
                        };
                        let exports = self.module_exports.entry(key.clone()).or_default();
                        if exports.constants.contains_key(&function.name)
                            || exports.structs.contains_key(&function.name)
                            || exports.enums.contains_key(&function.name)
                        {
                            let previous_location = exports
                                .constants
                                .get(&function.name)
                                .map(|entry| entry.span.start_location)
                                .or_else(|| {
                                    exports
                                        .structs
                                        .get(&function.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .or_else(|| {
                                    exports
                                        .enums
                                        .get(&function.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .unwrap_or(function.span.start_location);
                            self.errors.push(SemanticError::new(
                                format!(
                                    "function '{}' conflicts with existing exported symbol in module '{}' (previous definition at {})",
                                    function.name, key, previous_location
                                ),
                                function.span,
                            ));
                            continue;
                        }
                        match exports.functions.entry(function.name.clone()) {
                            Entry::Vacant(entry) => {
                                entry.insert(FunctionExport {
                                    signature: placeholder_signature,
                                    span: function.span,
                                });
                            }
                            Entry::Occupied(entry) => {
                                let previous_location = entry.get().span.start_location;
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "function '{}' already defined in module '{}' (previous definition at {})",
                                        function.name, key, previous_location
                                    ),
                                    function.span,
                                ));
                            }
                        }
                    }
                    Item::Struct(struct_decl) => {
                        if struct_decl.visibility != Visibility::Public {
                            continue;
                        }

                        let exports = self.module_exports.entry(key.clone()).or_default();
                        if let Some(conflict_span) =
                            Self::export_conflict_span(&*exports, &struct_decl.name)
                        {
                            let previous_location = conflict_span.start_location;
                            self.errors.push(SemanticError::new(
                                format!(
                                    "struct '{}' conflicts with existing exported symbol in module '{}' (previous definition at {})",
                                    struct_decl.name, key, previous_location
                                ),
                                struct_decl.span,
                            ));
                            continue;
                        }

                        exports.structs.insert(
                            struct_decl.name.clone(),
                            StructExport {
                                ty: Type::Struct(struct_decl.name.clone(), HashMap::new()),
                                span: struct_decl.span,
                            },
                        );
                    }
                    Item::Constant(constant) => {
                        if constant.visibility != Visibility::Public {
                            continue;
                        }

                        let exports = self.module_exports.entry(key.clone()).or_default();
                        if let Some(conflict_span) =
                            Self::export_conflict_span(&*exports, &constant.name)
                        {
                            let previous_location = conflict_span.start_location;
                            self.errors.push(SemanticError::new(
                                format!(
                                    "constant '{}' conflicts with existing exported symbol in module '{}' (previous definition at {})",
                                    constant.name, key, previous_location
                                ),
                                constant.span,
                            ));
                            continue;
                        }

                        exports.constants.insert(
                            constant.name.clone(),
                            ConstantExport {
                                ty: Type::Unknown,
                                span: constant.span,
                            },
                        );
                    }
                    Item::Enum(enum_decl) => {
                        if enum_decl.visibility != Visibility::Public {
                            continue;
                        }
                        let exports = self.module_exports.entry(key.clone()).or_default();
                        if exports.functions.contains_key(&enum_decl.name)
                            || exports.constants.contains_key(&enum_decl.name)
                            || exports.structs.contains_key(&enum_decl.name)
                        {
                            let previous_location = exports
                                .functions
                                .get(&enum_decl.name)
                                .map(|entry| entry.span.start_location)
                                .or_else(|| {
                                    exports
                                        .constants
                                        .get(&enum_decl.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .or_else(|| {
                                    exports
                                        .structs
                                        .get(&enum_decl.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .unwrap_or(enum_decl.span.start_location);
                            self.errors.push(SemanticError::new(
                                format!(
                                    "enum '{}' conflicts with existing exported symbol in module '{}' (previous definition at {})",
                                    enum_decl.name, key, previous_location
                                ),
                                enum_decl.span,
                            ));
                            continue;
                        }
                        match exports.enums.entry(enum_decl.name.clone()) {
                            Entry::Vacant(entry) => {
                                entry.insert(EnumExport {
                                    ty: Type::Enum(enum_decl.name.clone()),
                                    variants: HashMap::new(),
                                    span: enum_decl.span,
                                });
                            }
                            Entry::Occupied(entry) => {
                                let previous_location = entry.get().span.start_location;
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "enum '{}' already defined in module '{}' (previous definition at {})",
                                        enum_decl.name, key, previous_location
                                    ),
                                    enum_decl.span,
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        for module in modules {
            let Some(path) = module.name.as_ref() else {
                continue;
            };
            let key = module_path_key(path);
            for item in &module.items {
                if let Item::Export(export) = item {
                    self.collect_reexport(&key, export);
                }
            }
        }
    }

    fn collect_reexport(&mut self, current_key: &str, export: &'a Export) {
        let target_key = module_path_key(&export.module_path);

        if !self.module_registry.contains_key(&target_key) {
            self.errors.push(SemanticError::new(
                format!("cannot export from unknown module '{}'", target_key),
                export.module_path.span,
            ));
            return;
        }

        // Clone the target exports to avoid borrow issues
        let target_exports = match self.module_exports.get(&target_key).cloned() {
            Some(exports) => exports,
            None => {
                self.errors.push(SemanticError::new(
                    format!("module '{}' has no exported symbols", target_key),
                    export.module_path.span,
                ));
                return;
            }
        };

        let current_exports = self
            .module_exports
            .entry(current_key.to_string())
            .or_default();

        for item in &export.items {
            let source_name = &item.name;
            let local_name = item.alias.as_ref().unwrap_or(source_name);

            if let Some(span) = Self::export_conflict_span(current_exports, local_name) {
                self.errors.push(SemanticError::new(
                    format!(
                        "exported symbol '{}' already defined (previous definition at {})",
                        local_name, span.start_location
                    ),
                    item.span,
                ));
                continue;
            }

            if let Some(function_export) = target_exports.functions.get(source_name) {
                current_exports.functions.insert(
                    local_name.clone(),
                    FunctionExport {
                        signature: function_export.signature.clone(),
                        span: item.span,
                    },
                );
                continue;
            }

            if let Some(constant_export) = target_exports.constants.get(source_name) {
                current_exports.constants.insert(
                    local_name.clone(),
                    ConstantExport {
                        ty: constant_export.ty.clone(),
                        span: item.span,
                    },
                );
                continue;
            }

            if let Some(struct_export) = target_exports.structs.get(source_name) {
                current_exports.structs.insert(
                    local_name.clone(),
                    StructExport {
                        ty: struct_export.ty.clone(),
                        span: item.span,
                    },
                );
                continue;
            }

            if let Some(enum_export) = target_exports.enums.get(source_name) {
                current_exports.enums.insert(
                    local_name.clone(),
                    EnumExport {
                        ty: enum_export.ty.clone(),
                        variants: enum_export.variants.clone(),
                        span: item.span,
                    },
                );
                continue;
            }

            self.errors.push(SemanticError::new(
                format!(
                    "module '{}' does not export symbol '{}'",
                    target_key, source_name
                ),
                item.span,
            ));
        }
    }

    fn export_conflict_span(exports: &ModuleExport, name: &str) -> Option<Span> {
        exports
            .functions
            .get(name)
            .map(|entry| entry.span)
            .or_else(|| exports.constants.get(name).map(|entry| entry.span))
            .or_else(|| exports.structs.get(name).map(|entry| entry.span))
            .or_else(|| exports.enums.get(name).map(|entry| entry.span))
    }

    fn introduce_imports(&mut self, module: &'a Module) {
        let mut seen_full_imports = HashSet::new();

        for item in &module.items {
            let Item::Import(import) = item else {
                continue;
            };

            let key = module_path_key(&import.path);

            if let Some(alias) = import.alias.as_ref() {
                let alias_info = ModuleAliasInfo {
                    alias: alias.clone(),
                    module_key: key.clone(),
                    span: import.span,
                };

                match self.scopes.define(
                    alias,
                    import.span,
                    SymbolKind::ModuleAlias,
                    Type::Module(alias_info.clone()),
                    false,
                ) {
                    Ok(()) => {
                        self.module_aliases.insert(alias.clone(), alias_info);
                    }
                    Err(previous_span) => {
                        let previous_location = previous_span.start_location;
                        self.errors.push(SemanticError::new(
                            format!(
                                "import alias '{}' conflicts with existing symbol (previous definition at {})",
                                alias, previous_location
                            ),
                            import.span,
                        ));
                    }
                }
            }

            if let Some(items) = import.items.as_ref() {
                if !items.is_empty() {
                    self.introduce_selective_import(import, &key);
                }
                continue;
            }

            if import.alias.is_some() {
                continue;
            }

            if self.current_module_key.as_deref() == Some(key.as_str()) {
                continue;
            }

            if !seen_full_imports.insert(key.clone()) {
                continue;
            }

            self.introduce_full_import(import, &key);
        }
    }

    fn introduce_full_import(&mut self, import: &'a Import, module_key: &str) {
        let Some(exports) = self.module_exports.get(module_key) else {
            return;
        };

        let function_exports = exports
            .functions
            .iter()
            .map(|(name, export)| (name.clone(), export.clone()))
            .collect::<Vec<_>>();
        let constant_exports = exports
            .constants
            .iter()
            .map(|(name, export)| (name.clone(), export.clone()))
            .collect::<Vec<_>>();
        let struct_exports = exports
            .structs
            .iter()
            .map(|(name, export)| (name.clone(), export.clone()))
            .collect::<Vec<_>>();
        let enum_exports = exports
            .enums
            .iter()
            .map(|(name, export)| (name.clone(), export.clone()))
            .collect::<Vec<_>>();

        let mut function_updates = Vec::new();
        for (name, export) in &function_exports {
            if let Err(previous_span) = self.scopes.define(
                name,
                export.span,
                SymbolKind::Function,
                Type::Function(export.signature.clone()),
                false,
            ) {
                let previous_location = previous_span.start_location;
                self.errors.push(SemanticError::new(
                    format!(
                        "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                        name, previous_location
                    ),
                    import.span,
                ));
            } else {
                self.current_module_imports.insert(name.clone());
                function_updates.push((name.clone(), export.signature.clone(), export.span));
            }
        }
        if let Some(current_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(current_key) {
                for (name, signature, span) in function_updates {
                    if let Some(entry) = exports.functions.get_mut(&name) {
                        entry.signature = signature.clone();
                        entry.span = span;
                    }
                }
            }
        }

        let mut constant_updates = Vec::new();
        for (name, export) in &constant_exports {
            if let Err(previous_span) = self.scopes.define(
                name,
                export.span,
                SymbolKind::Constant,
                export.ty.clone(),
                false,
            ) {
                let previous_location = previous_span.start_location;
                self.errors.push(SemanticError::new(
                    format!(
                        "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                        name, previous_location
                    ),
                    import.span,
                ));
            } else {
                self.current_module_imports.insert(name.clone());
                constant_updates.push((name.clone(), export.ty.clone(), export.span));
            }
        }
        if let Some(current_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(current_key) {
                for (name, ty, span) in constant_updates {
                    if let Some(entry) = exports.constants.get_mut(&name) {
                        entry.ty = ty.clone();
                        entry.span = span;
                    }
                }
            }
        }

        let mut struct_updates = Vec::new();
        for (name, export) in &struct_exports {
            if let Err(previous_span) = self.scopes.define(
                name,
                export.span,
                SymbolKind::Constant,
                export.ty.clone(),
                false,
            ) {
                let previous_location = previous_span.start_location;
                self.errors.push(SemanticError::new(
                    format!(
                        "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                        name, previous_location
                    ),
                    import.span,
                ));
            } else {
                self.struct_registry.insert(name.clone(), export.ty.clone());
                self.current_module_imports.insert(name.clone());
                struct_updates.push((name.clone(), export.ty.clone(), export.span));
            }
        }
        if let Some(current_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(current_key) {
                for (name, ty, span) in struct_updates {
                    if let Some(entry) = exports.structs.get_mut(&name) {
                        entry.ty = ty.clone();
                        entry.span = span;
                    }
                }
            }
        }

        let mut enum_updates = Vec::new();
        for (name, export) in &enum_exports {
            if let Err(previous_span) = self.scopes.define(
                name,
                export.span,
                SymbolKind::Constant,
                export.ty.clone(),
                false,
            ) {
                let previous_location = previous_span.start_location;
                self.errors.push(SemanticError::new(
                    format!(
                        "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                        name, previous_location
                    ),
                    import.span,
                ));
            } else {
                self.current_module_imports.insert(name.clone());
                enum_updates.push((name.clone(), export.ty.clone(), export.span));
            }
        }
        if let Some(current_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(current_key) {
                for (name, ty, span) in enum_updates {
                    if let Some(entry) = exports.enums.get_mut(&name) {
                        entry.ty = ty.clone();
                        entry.span = span;
                    }
                }
            }
        }
    }

    fn introduce_selective_import(&mut self, import: &'a Import, module_key: &str) {
        let Some(items) = import.items.as_ref() else {
            return;
        };
        let Some(exports) = self.module_exports.get(module_key) else {
            return;
        };

        let mut function_updates = Vec::new();
        let mut constant_updates = Vec::new();
        let mut struct_updates = Vec::new();
        let mut enum_updates = Vec::new();

        for item in items {
            let source_name = &item.name;
            let local_name = item.alias.as_ref().unwrap_or(source_name);

            if let Some(export) = exports.functions.get(source_name).cloned() {
                if let Err(previous_span) = self.scopes.define(
                    local_name,
                    item.span,
                    SymbolKind::Function,
                    Type::Function(export.signature.clone()),
                    false,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                            local_name, previous_location
                        ),
                        item.span,
                    ));
                } else {
                    self.current_module_imports.insert(local_name.clone());
                    function_updates.push((local_name.to_string(), export.signature, export.span));
                }
                continue;
            }

            if let Some(export) = exports.constants.get(source_name).cloned() {
                if let Err(previous_span) = self.scopes.define(
                    local_name,
                    item.span,
                    SymbolKind::Constant,
                    export.ty.clone(),
                    false,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                            local_name, previous_location
                        ),
                        item.span,
                    ));
                } else {
                    self.current_module_imports.insert(local_name.clone());
                    constant_updates.push((local_name.to_string(), export.ty, export.span));
                }
                continue;
            }

            if let Some(export) = exports.structs.get(source_name).cloned() {
                if let Err(previous_span) = self.scopes.define(
                    local_name,
                    item.span,
                    SymbolKind::Constant,
                    export.ty.clone(),
                    false,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                            local_name, previous_location
                        ),
                        item.span,
                    ));
                } else {
                    self.struct_registry
                        .insert(local_name.clone(), export.ty.clone());
                    self.current_module_imports.insert(local_name.clone());
                    struct_updates.push((local_name.to_string(), export.ty, export.span));
                }
                continue;
            }

            if let Some(export) = exports.enums.get(source_name).cloned() {
                if let Err(previous_span) = self.scopes.define(
                    local_name,
                    item.span,
                    SymbolKind::Constant,
                    export.ty.clone(),
                    false,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                            local_name, previous_location
                        ),
                        item.span,
                    ));
                } else {
                    self.current_module_imports.insert(local_name.clone());
                    enum_updates.push((local_name.to_string(), export.ty, export.span));
                }
                continue;
            }

            self.errors.push(SemanticError::new(
                format!(
                    "module '{}' does not export symbol '{}'",
                    module_key, source_name
                ),
                item.span,
            ));
        }

        if let Some(current_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(current_key) {
                for (name, signature, span) in function_updates {
                    if let Some(entry) = exports.functions.get_mut(&name) {
                        entry.signature = signature.clone();
                        entry.span = span;
                    }
                }
                for (name, ty, span) in constant_updates {
                    if let Some(entry) = exports.constants.get_mut(&name) {
                        entry.ty = ty.clone();
                        entry.span = span;
                    }
                }
                for (name, ty, span) in struct_updates {
                    if let Some(entry) = exports.structs.get_mut(&name) {
                        entry.ty = ty.clone();
                        entry.span = span;
                    }
                }
                for (name, ty, span) in enum_updates {
                    if let Some(entry) = exports.enums.get_mut(&name) {
                        entry.ty = ty.clone();
                        entry.span = span;
                    }
                }
            }
        }
    }

    fn analyze_module(&mut self, module: &'a Module) {
        self.struct_registry.clear();
        self.current_module_imports.clear();
        self.scopes.push(ScopeKind::Module);
        self.current_module_key = module.name.as_ref().map(module_path_key);
        self.current_module_signatures.clear();
        self.register_structs(module);
        self.register_enums(module);
        self.validate_imports(module);
        self.introduce_imports(module);
        self.register_functions(module);
        self.register_constants(module);

        for item in &module.items {
            match item {
                Item::Function(function) => self.analyze_function(function),
                Item::Constant(constant) => self.analyze_constant(constant),
                Item::Struct(struct_decl) => self.analyze_struct(struct_decl),
                Item::Enum(enum_decl) => self.analyze_enum(enum_decl),
                Item::Stmt(statement) => self.analyze_statement(statement),
                Item::Import(_) | Item::Export(_) => {}
            }
        }

        self.validate_entry_points(module);

        if let Some(frame) = self.scopes.pop() {
            self.report_unused(frame);
        }

        self.current_module_key = None;
        self.current_module_signatures.clear();
    }

    fn register_structs(&mut self, module: &'a Module) {
        for item in &module.items {
            if let Item::Struct(struct_decl) = item {
                let struct_type = Type::Struct(struct_decl.name.clone(), HashMap::new());
                self.struct_registry
                    .insert(struct_decl.name.clone(), struct_type.clone());

                if let Err(previous_span) = self.scopes.define(
                    &struct_decl.name,
                    struct_decl.span,
                    SymbolKind::Constant,
                    struct_type,
                    false,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "struct '{}' already defined (previous definition at {})",
                            struct_decl.name, previous_location
                        ),
                        struct_decl.span,
                    ));
                }
            }
        }
    }

    fn register_enums(&mut self, module: &'a Module) {
        for item in &module.items {
            if let Item::Enum(enum_decl) = item {
                let enum_type = Type::Enum(enum_decl.name.clone());

                if let Err(previous_span) = self.scopes.define(
                    &enum_decl.name,
                    enum_decl.span,
                    SymbolKind::Constant,
                    enum_type,
                    false,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "enum '{}' already defined (previous definition at {})",
                            enum_decl.name, previous_location
                        ),
                        enum_decl.span,
                    ));
                }
            }
        }
    }

    fn register_functions(&mut self, module: &'a Module) {
        for item in &module.items {
            if let Item::Function(function) = item {
                let signature = self.build_function_signature(function);
                match self.scopes.define(
                    &function.name,
                    function.span,
                    SymbolKind::Function,
                    Type::Function(signature.clone()),
                    false,
                ) {
                    Err(previous_span) => {
                        let previous_location = previous_span.start_location;
                        if self.current_module_imports.contains(&function.name) {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "imported symbol '{}' conflicts with local definition (previous definition at {})",
                                    function.name, previous_location
                                ),
                                function.span,
                            ));
                        } else {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "function '{}' already defined (previous definition at {})",
                                    function.name, previous_location
                                ),
                                function.span,
                            ));
                        }
                    }
                    Ok(()) => {
                        self.current_module_signatures
                            .insert(function.name.clone(), signature.clone());
                        if let Some(current_key) = self.current_module_key.as_ref() {
                            if let Some(exports) = self.module_exports.get_mut(current_key) {
                                if let Some(entry) = exports.functions.get_mut(&function.name) {
                                    entry.signature = signature.clone();
                                    entry.span = function.span;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn register_constants(&mut self, module: &'a Module) {
        for item in &module.items {
            if let Item::Constant(constant) = item {
                if let Err(previous_span) = self.scopes.define(
                    &constant.name,
                    constant.span,
                    SymbolKind::Constant,
                    Type::Unknown,
                    constant.mutable,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "binding '{}' already defined (previous definition at {})",
                            constant.name, previous_location
                        ),
                        constant.span,
                    ));
                }
            }
        }
    }

    fn validate_entry_points(&mut self, module: &'a Module) {
        if module.name.is_none() {
            return;
        }

        for item in &module.items {
            let Item::Function(function) = item else {
                continue;
            };

            if function.name != "main" {
                continue;
            }

            if let Some(param) = function.parameters.first() {
                self.errors.push(SemanticError::new(
                    "entry point 'main' must not accept parameters",
                    param.span,
                ));
            }

            match function.return_type.as_ref() {
                Some(ty) => {
                    let signature = self
                        .current_module_signatures
                        .get(&function.name)
                        .cloned()
                        .unwrap_or_else(|| self.build_function_signature(function));
                    if !matches!(signature.return_type.as_ref(), Type::Integer) {
                        let declared = ty.segments.join("::");
                        self.errors.push(SemanticError::new(
                            format!(
                                "entry point 'main' must return i32 but returns {}",
                                declared
                            ),
                            ty.span,
                        ));
                    }
                }
                None => {
                    self.errors.push(SemanticError::new(
                        "entry point 'main' must declare a return type of i32",
                        function.span,
                    ));
                }
            }
        }
    }

    fn validate_imports(&mut self, module: &'a Module) {
        let current_key = module.name.as_ref().map(module_path_key);
        for item in &module.items {
            if let Item::Import(import) = item {
                self.validate_import(import, current_key.as_deref());
            }
        }
    }

    fn validate_import(&mut self, import: &Import, current_module: Option<&str>) {
        let key = module_path_key(&import.path);
        if let Some(current) = current_module {
            if current == key {
                self.errors.push(SemanticError::new(
                    "module cannot import itself",
                    import.span,
                ));
            }
        }
        if !self.module_registry.contains_key(&key) {
            self.errors.push(SemanticError::new(
                format!("unknown module '{}'", key),
                import.span,
            ));
            return;
        }

        if let Some(items) = import.items.as_ref() {
            let Some(exports) = self.module_exports.get(&key) else {
                self.errors.push(SemanticError::new(
                    format!("module '{}' has no exported symbols", key),
                    import.span,
                ));
                return;
            };

            for item in items {
                let name = &item.name;
                let exists = exports.functions.contains_key(name)
                    || exports.constants.contains_key(name)
                    || exports.structs.contains_key(name)
                    || exports.enums.contains_key(name);
                if !exists {
                    self.errors.push(SemanticError::new(
                        format!("module '{}' does not export symbol '{}'", key, name),
                        item.span,
                    ));
                }
            }
        }
    }

    fn analyze_function(&mut self, function: &'a Function) {
        self.scopes.push(ScopeKind::Function);
        let signature = self
            .current_module_signatures
            .get(&function.name)
            .cloned()
            .unwrap_or_else(|| self.build_function_signature(function));
        let mut value_return_type = (*signature.return_type).clone();
        if function.is_async {
            value_return_type = match value_return_type {
                Type::Future(inner) => *inner,
                other => {
                    self.errors.push(SemanticError::new(
                        format!(
                            "async function '{}' must return a future type",
                            function.name
                        ),
                        function.span,
                    ));
                    other
                }
            };
        }
        self.current_function = Some(FunctionContext {
            name: &function.name,
            span: function.span,
            return_type: value_return_type.clone(),
            saw_value_return: false,
            is_async: function.is_async,
        });

        for (parameter, param_type) in function.parameters.iter().zip(signature.parameters.iter()) {
            if *param_type == Type::Void {
                self.errors.push(SemanticError::new(
                    format!("parameter '{}' cannot have type void", parameter.name),
                    parameter.span,
                ));
            }
            if let Err(previous_span) = self.scopes.define(
                &parameter.name,
                parameter.span,
                SymbolKind::Parameter,
                param_type.clone(),
                false,
            ) {
                let previous_location = previous_span.start_location;
                self.errors.push(SemanticError::new(
                    format!(
						"parameter '{}' already defined in function '{}' (previous definition at {})",
						parameter.name, function.name, previous_location
					),
                    parameter.span,
                ));
            }
        }

        self.analyze_block(&function.body);

        if let Some(context) = self.current_function.take() {
            if context.return_type != Type::Void && !context.saw_value_return {
                self.errors.push(SemanticError::new(
                    format!(
                        "function '{}' must return a value of type {} in all paths",
                        context.name,
                        context.return_type.describe()
                    ),
                    context.span,
                ));
            }
        }

        if let Some(frame) = self.scopes.pop() {
            self.report_unused(frame);
        }
    }

    fn analyze_constant(&mut self, constant: &'a Constant) {
        let value_type = self.analyze_expr(&constant.value);
        let annotated_type = constant
            .ty
            .as_ref()
            .map(|type_name| self.resolve_type_name(type_name));

        if constant.visibility == Visibility::Public && constant.mutable {
            self.errors.push(SemanticError::new(
                format!(
                    "exported binding '{}' cannot be mutable; use 'let' instead of 'var'",
                    constant.name
                ),
                constant.span,
            ));
        }

        if let Some(expected_type) = annotated_type.as_ref() {
            if !types_compatible(expected_type, &value_type) {
                self.errors.push(SemanticError::new(
                    format!(
                        "cannot assign value of type {} to binding '{}' annotated as {}",
                        value_type.describe(),
                        constant.name,
                        expected_type.describe()
                    ),
                    expr_span(&constant.value),
                ));
            }
        }

        let resolved_type = annotated_type.clone().unwrap_or_else(|| value_type.clone());

        if !self
            .scopes
            .set_symbol_type(&constant.name, resolved_type.clone())
        {
            if let Err(previous_span) = self.scopes.define(
                &constant.name,
                constant.span,
                SymbolKind::Constant,
                resolved_type.clone(),
                constant.mutable,
            ) {
                let previous_location = previous_span.start_location;
                self.errors.push(SemanticError::new(
                    format!(
                        "binding '{}' already defined (previous definition at {})",
                        constant.name, previous_location
                    ),
                    constant.span,
                ));
            }
        }

        if let Some(module_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(module_key) {
                if let Some(entry) = exports.constants.get_mut(&constant.name) {
                    entry.ty = resolved_type;
                }
            }
        }
    }

    fn analyze_struct(&mut self, struct_decl: &'a crate::ast::StructDecl) {
        let mut fields = HashMap::new();

        for field in &struct_decl.fields {
            let field_type = self.resolve_type_name(&field.ty);

            if field_type == Type::Void {
                self.errors.push(SemanticError::new(
                    format!("field '{}' cannot have type void", field.name),
                    field.span,
                ));
            }

            if fields.contains_key(&field.name) {
                self.errors.push(SemanticError::new(
                    format!(
                        "duplicate field '{}' in struct '{}'",
                        field.name, struct_decl.name
                    ),
                    field.span,
                ));
            } else {
                fields.insert(field.name.clone(), field_type);
            }
        }

        // Update the already-registered struct with actual field types
        let struct_type = Type::Struct(struct_decl.name.clone(), fields);
        self.struct_registry
            .insert(struct_decl.name.clone(), struct_type.clone());

        // Note: The struct was already registered in register_structs,
        // so we don't call define again to avoid duplicate definition errors

        if let Some(module_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(module_key) {
                if let Some(entry) = exports.structs.get_mut(&struct_decl.name) {
                    entry.ty = struct_type.clone();
                }
            }
        }
    }

    fn analyze_enum(&mut self, enum_decl: &'a crate::ast::EnumDecl) {
        use std::collections::HashSet;
        let mut variant_names = HashSet::new();
        let mut variants_info = HashMap::new();

        for variant in &enum_decl.variants {
            if !variant_names.insert(variant.name.clone()) {
                self.errors.push(SemanticError::new(
                    format!(
                        "duplicate variant '{}' in enum '{}'",
                        variant.name, enum_decl.name
                    ),
                    variant.span,
                ));
            }

            let variant_info = match variant.data.as_ref() {
                None => EnumVariantInfo::Unit,
                Some(crate::ast::EnumVariantData::Tuple(types)) => {
                    let mut resolved_types = Vec::new();
                    for ty in types {
                        let resolved_type = self.resolve_type_name(ty);
                        if resolved_type == Type::Void {
                            self.errors.push(SemanticError::new(
                                format!("enum variant '{}' cannot contain type void", variant.name),
                                variant.span,
                            ));
                        }
                        resolved_types.push(resolved_type);
                    }
                    EnumVariantInfo::Tuple(resolved_types)
                }
                Some(crate::ast::EnumVariantData::Struct(fields)) => {
                    let mut field_names = HashSet::new();
                    let mut field_types = HashMap::new();
                    for field in fields {
                        let field_type = self.resolve_type_name(&field.ty);
                        if field_type == Type::Void {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "field '{}' in variant '{}' cannot have type void",
                                    field.name, variant.name
                                ),
                                field.span,
                            ));
                        }

                        if !field_names.insert(field.name.clone()) {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "duplicate field '{}' in variant '{}' of enum '{}'",
                                    field.name, variant.name, enum_decl.name
                                ),
                                field.span,
                            ));
                            continue;
                        }

                        field_types.insert(field.name.clone(), field_type);
                    }
                    EnumVariantInfo::Struct(field_types)
                }
            };

            variants_info
                .entry(variant.name.clone())
                .or_insert(variant_info);
        }

        if let Some(enum_info) = self.enum_registry.get_mut(&enum_decl.name) {
            enum_info.variants = variants_info.clone();
        } else {
            self.enum_registry.insert(
                enum_decl.name.clone(),
                EnumInfo {
                    variants: variants_info.clone(),
                },
            );
        }

        if let Some(module_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(module_key) {
                if let Some(entry) = exports.enums.get_mut(&enum_decl.name) {
                    entry.ty = Type::Enum(enum_decl.name.clone());
                    entry.variants = variants_info;
                }
            }
        }
    }

    fn build_function_signature(&mut self, function: &'a Function) -> FunctionType {
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| self.resolve_type_name(&parameter.ty))
            .collect();
        let resolved_return = function
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_name(ty))
            .unwrap_or(Type::Void);
        let return_type = if function.is_async {
            match resolved_return {
                Type::Future(_) => resolved_return,
                other => Type::Future(Box::new(other)),
            }
        } else {
            resolved_return
        };
        FunctionType {
            parameters,
            return_type: Box::new(return_type),
        }
    }

    fn analyze_block(&mut self, block: &Block) {
        self.scopes.push(ScopeKind::Block);
        for statement in &block.statements {
            self.analyze_statement(statement);
        }
        if let Some(frame) = self.scopes.pop() {
            self.report_unused(frame);
        }
    }

    fn analyze_statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                span,
                mutable,
            } => {
                let value_type = self.analyze_expr(value);
                let annotated_type = ty
                    .as_ref()
                    .map(|type_name| self.resolve_type_name(type_name));

                if let Some(expected_type) = annotated_type.as_ref() {
                    if !types_compatible(expected_type, &value_type) {
                        self.errors.push(SemanticError::new(
                            format!(
                                "cannot assign value of type {} to binding '{}' annotated as {}",
                                value_type.describe(),
                                name,
                                expected_type.describe()
                            ),
                            expr_span(value),
                        ));
                    }
                }

                let binding_type = annotated_type.unwrap_or_else(|| value_type.clone());

                if let Err(previous_span) =
                    self.scopes
                        .define(name, *span, SymbolKind::Variable, binding_type, *mutable)
                {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
							"variable '{}' already defined in this scope (previous definition at {})",
							name, previous_location
						),
                        *span,
                    ));
                }
            }
            Stmt::Assignment {
                target,
                value,
                span,
            } => {
                self.analyze_assignment(target, value, *span);
            }
            Stmt::IndexAssignment {
                array,
                index,
                value,
                span,
            } => {
                self.analyze_index_assignment(array, index, value, *span);
            }
            Stmt::FieldAssignment {
                object,
                field,
                value,
                span,
            } => {
                // Check that object has the field
                let object_type = self.analyze_expr(object);
                match &object_type {
                    Type::Struct(_, fields) => match fields.get(field) {
                        Some(field_type) => {
                            let value_type = self.analyze_expr(value);
                            if !types_compatible(field_type, &value_type) {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "field '{}' expects type {}, got {}",
                                        field,
                                        field_type.describe(),
                                        value_type.describe()
                                    ),
                                    *span,
                                ));
                            }
                        }
                        None => {
                            self.errors.push(SemanticError::new(
                                format!("struct has no field named '{}'", field),
                                *span,
                            ));
                        }
                    },
                    Type::Unknown => {
                        // Just analyze the value to find any errors in it
                        self.analyze_expr(value);
                    }
                    _ => {
                        self.errors.push(SemanticError::new(
                            format!(
                                "cannot assign to field '{}' on non-struct type {}",
                                field,
                                object_type.describe()
                            ),
                            *span,
                        ));
                        self.analyze_expr(value);
                    }
                }
            }
            Stmt::Using {
                name,
                ty,
                value,
                span,
            } => {
                let value_type = self.analyze_expr(value);
                let annotated_type = ty
                    .as_ref()
                    .map(|type_name| self.resolve_type_name(type_name));

                if let Some(expected_type) = annotated_type.as_ref() {
                    if !types_compatible(expected_type, &value_type) {
                        self.errors.push(SemanticError::new(
                            format!(
                                "cannot assign value of type {} to using binding '{}' annotated as {}",
                                value_type.describe(),
                                name,
                                expected_type.describe()
                            ),
                            expr_span(value),
                        ));
                    }
                }

                let binding_type = annotated_type.unwrap_or_else(|| value_type.clone());

                if binding_type == Type::Void {
                    self.errors.push(SemanticError::new(
                        format!("using binding '{}' cannot have type void", name),
                        *span,
                    ));
                }

                if let Err(previous_span) =
                    self.scopes
                        .define(name, *span, SymbolKind::Variable, binding_type, false)
                {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "binding '{}' already defined (previous definition at {})",
                            name, previous_location
                        ),
                        *span,
                    ));
                }
            }
            Stmt::Defer { body, span } => {
                if self.current_function.is_none() {
                    self.errors.push(SemanticError::new(
                        "defer statement outside of function",
                        *span,
                    ));
                }
                self.analyze_block(body);
            }
            Stmt::Try { body, catch, .. } => {
                self.analyze_block(body);
                self.scopes.push(ScopeKind::Block);
                if let Some(binding) = catch.binding.as_ref() {
                    let binding_span = catch.binding_span.unwrap_or(catch.span);
                    if let Err(previous_span) = self.scopes.define(
                        binding,
                        binding_span,
                        SymbolKind::Variable,
                        Type::Unknown,
                        false,
                    ) {
                        let previous_location = previous_span.start_location;
                        self.errors.push(SemanticError::new(
                            format!(
                                "binding '{}' already defined (previous definition at {})",
                                binding, previous_location
                            ),
                            binding_span,
                        ));
                    }
                }
                self.analyze_block(&catch.body);
                if let Some(frame) = self.scopes.pop() {
                    self.report_unused(frame);
                }
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr);
            }
            Stmt::Return { value, span } => self.analyze_return(value.as_ref(), *span),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_type = self.analyze_expr(condition);
                if condition_type != Type::Bool && condition_type != Type::Unknown {
                    self.errors.push(SemanticError::new(
                        format!(
                            "condition of 'if' must have type bool, found {}",
                            condition_type.describe()
                        ),
                        expr_span(condition),
                    ));
                }
                self.analyze_statement(then_branch);
                if let Some(branch) = else_branch.as_ref() {
                    self.analyze_statement(branch);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let condition_type = self.analyze_expr(condition);
                if condition_type != Type::Bool && condition_type != Type::Unknown {
                    self.errors.push(SemanticError::new(
                        format!(
                            "condition of 'while' must have type bool, found {}",
                            condition_type.describe()
                        ),
                        expr_span(condition),
                    ));
                }
                self.loop_depth += 1;
                self.analyze_statement(body);
                self.loop_depth -= 1;
            }
            Stmt::For {
                initializer,
                condition,
                increment,
                body,
                ..
            } => {
                self.scopes.push(ScopeKind::Block);

                if let Some(init) = initializer.as_deref() {
                    self.analyze_statement(init);
                }

                if let Some(cond) = condition.as_ref() {
                    let condition_type = self.analyze_expr(cond);
                    if condition_type != Type::Bool && condition_type != Type::Unknown {
                        self.errors.push(SemanticError::new(
                            format!(
                                "condition of 'for' must have type bool, found {}",
                                condition_type.describe()
                            ),
                            expr_span(cond),
                        ));
                    }
                }

                if let Some(inc) = increment.as_ref() {
                    self.analyze_expr(inc);
                }

                self.loop_depth += 1;
                self.analyze_statement(body);
                self.loop_depth -= 1;

                if let Some(frame) = self.scopes.pop() {
                    self.report_unused(frame);
                }
            }
            Stmt::Match {
                expression, arms, ..
            } => {
                let expr_type = self.analyze_expr(expression);
                for arm in arms {
                    self.scopes.push(ScopeKind::Block);
                    self.analyze_match_pattern(&arm.pattern, &expr_type);
                    self.analyze_statement(&arm.body);
                    if let Some(frame) = self.scopes.pop() {
                        self.report_unused(frame);
                    }
                }
            }
            Stmt::Break { span } => {
                self.analyze_loop_control("break", *span);
            }
            Stmt::Continue { span } => {
                self.analyze_loop_control("continue", *span);
            }
            Stmt::Block(block) => self.analyze_block(block),
        }
    }

    fn analyze_match_pattern(&mut self, pattern: &MatchPattern, scrutinee_type: &Type) {
        match pattern {
            MatchPattern::Literal { value, span } => {
                let pattern_type = literal_type(value);
                if !types_compatible(scrutinee_type, &pattern_type) {
                    self.errors.push(SemanticError::new(
                        format!(
                            "match pattern type {} does not match expression type {}",
                            pattern_type.describe(),
                            scrutinee_type.describe()
                        ),
                        *span,
                    ));
                }
            }
            MatchPattern::Identifier { name, span } => {
                if name == "_" {
                    return;
                }
                if let Err(previous_span) = self.scopes.define(
                    name,
                    *span,
                    SymbolKind::Variable,
                    scrutinee_type.clone(),
                    false,
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "variable '{}' already defined in this scope (previous definition at {})",
                            name, previous_location
                        ),
                        *span,
                    ));
                }
            }
            MatchPattern::EnumVariant {
                enum_path,
                variant,
                kind,
                span,
            } => {
                self.analyze_enum_pattern(enum_path, variant, kind, *span, scrutinee_type);
            }
        }
    }

    fn analyze_enum_pattern(
        &mut self,
        enum_path: &[String],
        variant: &str,
        kind: &EnumPatternKind,
        span: Span,
        scrutinee_type: &Type,
    ) {
        if enum_path.len() != 1 {
            let mut qualified = enum_path.join("::");
            if qualified.is_empty() {
                qualified = variant.to_string();
            } else {
                qualified.push_str("::");
                qualified.push_str(variant);
            }
            self.errors.push(SemanticError::new(
                format!("qualified enum paths are not supported yet: {}", qualified),
                span,
            ));

            match kind {
                EnumPatternKind::Tuple(arguments) => {
                    for argument in arguments {
                        let unknown = Type::Unknown;
                        self.analyze_match_pattern(argument, &unknown);
                    }
                }
                EnumPatternKind::Struct(fields) => {
                    for field in fields {
                        let unknown = Type::Unknown;
                        self.analyze_match_pattern(&field.pattern, &unknown);
                    }
                }
                EnumPatternKind::Unit => {}
            }
            return;
        }

        let enum_name = &enum_path[0];

        let scrutinee_enum_name = match scrutinee_type {
            Type::Enum(name) => Some(name.clone()),
            Type::Unknown => None,
            other => {
                self.errors.push(SemanticError::new(
                    format!(
                        "match pattern uses enum variant '{}::{}' but expression has type {}",
                        enum_name,
                        variant,
                        other.describe()
                    ),
                    span,
                ));
                None
            }
        };

        if let Some(name) = scrutinee_enum_name.as_ref() {
            if enum_name != name {
                self.errors.push(SemanticError::new(
                    format!(
                        "match pattern references enum '{}' but expression has type enum {}",
                        enum_name, name
                    ),
                    span,
                ));
            }
        }

        let Some(enum_info) = self.enum_registry.get(enum_name) else {
            self.errors.push(SemanticError::new(
                format!("use of undeclared enum '{}' in match pattern", enum_name),
                span,
            ));
            return;
        };

        let Some(variant_info_ref) = enum_info.variants.get(variant) else {
            self.errors.push(SemanticError::new(
                format!("enum '{}' has no variant '{}'", enum_name, variant),
                span,
            ));
            return;
        };

        let variant_info = variant_info_ref.clone();
        let kind = kind.clone();

        match (variant_info, kind) {
            (EnumVariantInfo::Unit, EnumPatternKind::Unit) => {}
            (EnumVariantInfo::Unit, EnumPatternKind::Tuple(arguments)) => {
                for argument in &arguments {
                    let unknown = Type::Unknown;
                    self.analyze_match_pattern(argument, &unknown);
                }
                self.errors.push(SemanticError::new(
                    format!(
                        "enum variant '{}::{}' pattern does not accept arguments",
                        enum_name, variant
                    ),
                    span,
                ));
            }
            (EnumVariantInfo::Unit, EnumPatternKind::Struct(fields)) => {
                for field in &fields {
                    let unknown = Type::Unknown;
                    self.analyze_match_pattern(&field.pattern, &unknown);
                }
                self.errors.push(SemanticError::new(
                    format!(
                        "enum variant '{}::{}' pattern does not accept fields ({} provided)",
                        enum_name,
                        variant,
                        fields.len()
                    ),
                    span,
                ));
            }
            (EnumVariantInfo::Tuple(expected_types), EnumPatternKind::Tuple(arguments)) => {
                if expected_types.len() != arguments.len() {
                    self.errors.push(SemanticError::new(
                        format!(
                            "enum variant '{}::{}' pattern expects {} argument(s), got {}",
                            enum_name,
                            variant,
                            expected_types.len(),
                            arguments.len()
                        ),
                        span,
                    ));
                }

                for (index, argument) in arguments.iter().enumerate() {
                    if let Some(expected_type) = expected_types.get(index) {
                        self.analyze_match_pattern(argument, expected_type);
                    } else {
                        let unknown = Type::Unknown;
                        self.analyze_match_pattern(argument, &unknown);
                        self.errors.push(SemanticError::new(
                            format!(
                                "enum variant '{}::{}' does not accept argument {}",
                                enum_name,
                                variant,
                                index + 1
                            ),
                            pattern_span(argument),
                        ));
                    }
                }

                if expected_types.len() > arguments.len() {
                    for missing_index in arguments.len()..expected_types.len() {
                        self.errors.push(SemanticError::new(
                            format!(
                                "missing argument {} for enum variant '{}::{}' pattern",
                                missing_index + 1,
                                enum_name,
                                variant
                            ),
                            span,
                        ));
                    }
                }
            }
            (EnumVariantInfo::Tuple(expected_types), EnumPatternKind::Unit) => {
                self.errors.push(SemanticError::new(
                    format!(
                        "enum variant '{}::{}' pattern expects {} argument(s), but none were provided",
                        enum_name,
                        variant,
                        expected_types.len()
                    ),
                    span,
                ));
            }
            (EnumVariantInfo::Tuple(_expected_types), EnumPatternKind::Struct(fields)) => {
                for field in &fields {
                    let unknown = Type::Unknown;
                    self.analyze_match_pattern(&field.pattern, &unknown);
                }
                self.errors.push(SemanticError::new(
                    format!(
                        "enum variant '{}::{}' pattern expects positional arguments, got struct pattern with {} field(s)",
                        enum_name,
                        variant,
                        fields.len()
                    ),
                    span,
                ));
            }
            (EnumVariantInfo::Struct(expected_fields), EnumPatternKind::Struct(field_patterns)) => {
                let mut seen = HashSet::new();
                let mut present = HashSet::new();

                for field in &field_patterns {
                    if !seen.insert(field.name.clone()) {
                        self.errors.push(SemanticError::new(
                            format!(
                                "duplicate field '{}' in pattern for enum variant '{}::{}'",
                                field.name, enum_name, variant
                            ),
                            field.span,
                        ));
                        continue;
                    }

                    if let Some(expected_type) = expected_fields.get(&field.name) {
                        present.insert(field.name.clone());
                        self.analyze_match_pattern(&field.pattern, expected_type);
                    } else {
                        let unknown = Type::Unknown;
                        self.analyze_match_pattern(&field.pattern, &unknown);
                        self.errors.push(SemanticError::new(
                            format!(
                                "enum variant '{}::{}' has no field '{}'",
                                enum_name, variant, field.name
                            ),
                            field.span,
                        ));
                    }
                }

                for field_name in expected_fields.keys() {
                    if !present.contains(field_name) {
                        self.errors.push(SemanticError::new(
                            format!(
                                "missing field '{}' in pattern for enum variant '{}::{}'",
                                field_name, enum_name, variant
                            ),
                            span,
                        ));
                    }
                }
            }
            (EnumVariantInfo::Struct(_), EnumPatternKind::Tuple(arguments)) => {
                for argument in &arguments {
                    let unknown = Type::Unknown;
                    self.analyze_match_pattern(argument, &unknown);
                }
                self.errors.push(SemanticError::new(
                    format!(
                        "enum variant '{}::{}' pattern expects named fields, got positional pattern with {} argument(s)",
                        enum_name,
                        variant,
                        arguments.len()
                    ),
                    span,
                ));
            }
            (EnumVariantInfo::Struct(_), EnumPatternKind::Unit) => {
                self.errors.push(SemanticError::new(
                    format!(
                        "enum variant '{}::{}' pattern expects named fields, but none were provided",
                        enum_name,
                        variant
                    ),
                    span,
                ));
            }
        }
    }

    fn analyze_return(&mut self, value: Option<&Expr>, span: Span) {
        let Some(context_ref) = self.current_function.as_ref() else {
            if let Some(expr) = value {
                self.analyze_expr(expr);
            }
            self.errors.push(SemanticError::new(
                "return statement outside of function",
                span,
            ));
            return;
        };

        let context_name = context_ref.name.to_string();
        let return_type = context_ref.return_type.clone();

        if return_type == Type::Void {
            if let Some(expr) = value {
                let expr_type = self.analyze_expr(expr);
                self.errors.push(SemanticError::new(
                    format!(
                        "return statement in function '{}' cannot return a value (found {})",
                        context_name,
                        expr_type.describe()
                    ),
                    expr_span(expr),
                ));
            }
            return;
        }

        match value {
            Some(expr) => {
                let expr_type = self.analyze_expr(expr);
                if !types_compatible(&return_type, &expr_type) {
                    self.errors.push(SemanticError::new(
                        format!(
                            "return type mismatch in function '{}': expected {}, found {}",
                            context_name,
                            return_type.describe(),
                            expr_type.describe()
                        ),
                        expr_span(expr),
                    ));
                }
                if let Some(context) = self.current_function.as_mut() {
                    context.saw_value_return = true;
                }
            }
            None => {
                self.errors.push(SemanticError::new(
                    format!(
                        "return statement in function '{}' requires a value of type {}",
                        context_name,
                        return_type.describe()
                    ),
                    span,
                ));
            }
        }
    }

    fn analyze_assignment(&mut self, target: &str, value: &Expr, span: Span) {
        let value_type = self.analyze_expr(value);
        let mut diagnostics = Vec::new();

        {
            let Some(symbol) = self.scopes.resolve_mut_internal(target, true) else {
                self.errors.push(SemanticError::new(
                    format!("use of undeclared identifier '{}'", target),
                    span,
                ));
                return;
            };

            match symbol.kind {
                SymbolKind::Function => {
                    diagnostics.push(format!("cannot assign to function '{}'", target));
                }
                SymbolKind::ModuleAlias => {
                    diagnostics.push(format!("cannot assign to module alias '{}'", target));
                }
                SymbolKind::Variable | SymbolKind::Parameter | SymbolKind::Constant => {
                    if !symbol.mutable {
                        diagnostics
                            .push(format!("cannot assign to immutable binding '{}'", target));
                    }

                    if !types_compatible(&symbol.ty, &value_type) {
                        diagnostics.push(format!(
                            "cannot assign value of type {} to binding '{}' with type {}",
                            value_type.describe(),
                            target,
                            symbol.ty.describe()
                        ));
                    }

                    if symbol.ty == Type::Unknown && value_type != Type::Unknown {
                        symbol.ty = value_type.clone();
                    }
                }
            }
        }

        for message in diagnostics {
            self.errors.push(SemanticError::new(message, span));
        }
    }

    fn analyze_index_assignment(&mut self, array: &Expr, index: &Expr, value: &Expr, span: Span) {
        let array_type = self.analyze_expr(array);
        let index_type = self.analyze_expr(index);
        let value_type = self.analyze_expr(value);

        if !matches!(index_type, Type::Integer | Type::Unknown) {
            self.errors.push(SemanticError::new(
                format!("array index must be integer, got {}", index_type.describe()),
                expr_span(index),
            ));
        }

        match array_type {
            Type::Array { element, length } => {
                let element_type = (*element).clone();
                if !types_compatible(&element_type, &value_type) {
                    self.errors.push(SemanticError::new(
                        format!(
                            "array element expects type {}, got {}",
                            element_type.describe(),
                            value_type.describe()
                        ),
                        span,
                    ));
                }

                self.check_index_bounds(length, index, expr_span(index));
            }
            Type::Unknown => {}
            other => {
                self.errors.push(SemanticError::new(
                    format!("cannot assign through index on type {}", other.describe()),
                    expr_span(array),
                ));
            }
        }

        if let Some(name) = binding_name_from_expr(array) {
            if let Some(symbol) = self.scopes.resolve_mut_internal(name, true) {
                match symbol.kind {
                    SymbolKind::Variable | SymbolKind::Parameter | SymbolKind::Constant => {
                        if !symbol.mutable {
                            self.errors.push(SemanticError::new(
                                format!("cannot assign to immutable binding '{}'", name),
                                span,
                            ));
                        }
                    }
                    SymbolKind::Function => {
                        self.errors.push(SemanticError::new(
                            format!("cannot assign to function '{}'", name),
                            span,
                        ));
                    }
                    SymbolKind::ModuleAlias => {
                        self.errors.push(SemanticError::new(
                            format!("cannot assign to module alias '{}'", name),
                            span,
                        ));
                    }
                }
            }
        }
    }

    fn check_index_bounds(&mut self, length: Option<usize>, index_expr: &Expr, span: Span) {
        let Some(length) = length else {
            return;
        };

        let Some((value, literal_span)) = integer_constant(index_expr) else {
            return;
        };

        if value < 0 {
            self.errors.push(SemanticError::new(
                "array index cannot be negative".to_string(),
                literal_span,
            ));
            return;
        }

        let index_value = match usize::try_from(value) {
            Ok(value) => value,
            Err(_) => {
                self.errors.push(SemanticError::new(
                    "array index is too large".to_string(),
                    literal_span,
                ));
                return;
            }
        };
        if index_value >= length {
            self.errors.push(SemanticError::new(
                format!(
                    "array index {} out of bounds for length {}",
                    index_value, length
                ),
                span,
            ));
        }
    }

    fn analyze_loop_control(&mut self, keyword: &str, span: Span) {
        if self.loop_depth == 0 {
            self.errors.push(SemanticError::new(
                format!("{} statement outside of loop", keyword),
                span,
            ));
        }
    }

    fn analyze_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal { value, .. } => literal_type(value),
            Expr::Identifier { name, span } => match self.scopes.resolve(name) {
                Some(symbol_type) => symbol_type,
                None => {
                    self.errors.push(SemanticError::new(
                        format!("use of undeclared identifier '{}'", name),
                        *span,
                    ));
                    Type::Unknown
                }
            },
            Expr::Unary {
                operator,
                operand,
                span,
            } => {
                let operand_type = self.analyze_expr(operand);
                match operator {
                    UnaryOperator::Not => {
                        if operand_type != Type::Bool && operand_type != Type::Unknown {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "operator '{}' expects a bool operand but found {}",
                                    unary_operator_symbol(*operator),
                                    operand_type.describe()
                                ),
                                *span,
                            ));
                        }
                        Type::Bool
                    }
                    UnaryOperator::Negate => {
                        if !operand_type.is_numeric() && operand_type != Type::Unknown {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "operator '{}' expects a numeric operand but found {}",
                                    unary_operator_symbol(*operator),
                                    operand_type.describe()
                                ),
                                *span,
                            ));
                            Type::Unknown
                        } else {
                            operand_type
                        }
                    }
                }
            }
            Expr::Await { expression, span } => {
                let value_type = self.analyze_expr(expression);
                if let Some(context) = self.current_function.as_ref() {
                    if !context.is_async {
                        self.errors.push(SemanticError::new(
                            "'await' used outside of async function",
                            *span,
                        ));
                    }
                } else {
                    self.errors.push(SemanticError::new(
                        "'await' used outside of function",
                        *span,
                    ));
                }

                match value_type {
                    Type::Future(inner) => *inner,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.errors.push(SemanticError::new(
                            format!("'await' expects a future but found {}", other.describe()),
                            *span,
                        ));
                        Type::Unknown
                    }
                }
            }
            Expr::Binary {
                left,
                operator,
                right,
                span,
            } => {
                let left_type = self.analyze_expr(left);
                let right_type = self.analyze_expr(right);
                match operator {
                    BinaryOperator::Add
                    | BinaryOperator::Sub
                    | BinaryOperator::Mul
                    | BinaryOperator::Div
                    | BinaryOperator::Mod => {
                        if (!left_type.is_numeric() && left_type != Type::Unknown)
                            || (!right_type.is_numeric() && right_type != Type::Unknown)
                        {
                            if !left_type.is_numeric() && left_type != Type::Unknown {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "left operand of '{}' must be numeric, found {}",
                                        binary_operator_symbol(*operator),
                                        left_type.describe()
                                    ),
                                    *span,
                                ));
                            }
                            if !right_type.is_numeric() && right_type != Type::Unknown {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "right operand of '{}' must be numeric, found {}",
                                        binary_operator_symbol(*operator),
                                        right_type.describe()
                                    ),
                                    *span,
                                ));
                            }
                            Type::Unknown
                        } else if left_type != right_type
                            && left_type != Type::Unknown
                            && right_type != Type::Unknown
                        {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "operands of '{}' must have matching numeric types, found {} and {}",
                                    binary_operator_symbol(*operator),
                                        left_type.describe(),
                                        right_type.describe()
                                ),
                                *span,
                            ));
                            Type::Unknown
                        } else if left_type == Type::Unknown {
                            right_type
                        } else {
                            left_type
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        if left_type != right_type
                            && left_type != Type::Unknown
                            && right_type != Type::Unknown
                        {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "operands of '{}' must have the same type, found {} and {}",
                                    binary_operator_symbol(*operator),
                                    left_type.describe(),
                                    right_type.describe()
                                ),
                                *span,
                            ));
                        }
                        Type::Bool
                    }
                    BinaryOperator::Greater
                    | BinaryOperator::GreaterEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessEqual => {
                        if (!left_type.is_numeric() && left_type != Type::Unknown)
                            || (!right_type.is_numeric() && right_type != Type::Unknown)
                        {
                            if !left_type.is_numeric() && left_type != Type::Unknown {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "left operand of '{}' must be numeric, found {}",
                                        binary_operator_symbol(*operator),
                                        left_type.describe()
                                    ),
                                    *span,
                                ));
                            }
                            if !right_type.is_numeric() && right_type != Type::Unknown {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "right operand of '{}' must be numeric, found {}",
                                        binary_operator_symbol(*operator),
                                        right_type.describe()
                                    ),
                                    *span,
                                ));
                            }
                        }
                        if left_type != right_type
                            && left_type != Type::Unknown
                            && right_type != Type::Unknown
                        {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "operands of '{}' must have matching numeric types, found {} and {}",
                                    binary_operator_symbol(*operator),
                                        left_type.describe(),
                                        right_type.describe()
                                ),
                                *span,
                            ));
                        }
                        Type::Bool
                    }
                    BinaryOperator::And | BinaryOperator::Or => {
                        // Both operands must be boolean
                        if left_type != Type::Bool && left_type != Type::Unknown {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "left operand of '{}' must be boolean, found {}",
                                    binary_operator_symbol(*operator),
                                    left_type.describe()
                                ),
                                *span,
                            ));
                        }
                        if right_type != Type::Bool && right_type != Type::Unknown {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "right operand of '{}' must be boolean, found {}",
                                    binary_operator_symbol(*operator),
                                    right_type.describe()
                                ),
                                *span,
                            ));
                        }
                        Type::Bool
                    }
                }
            }
            Expr::Call {
                callee,
                arguments,
                span,
            } => {
                let callee_type = self.analyze_expr(callee);
                let argument_info: Vec<(Type, Span)> = arguments
                    .iter()
                    .map(|argument| (self.analyze_expr(argument), expr_span(argument)))
                    .collect();
                match callee_type {
                    Type::Function(signature) => {
                        if signature.parameters.len() != argument_info.len() {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "function expects {} argument(s) but {} provided",
                                    signature.parameters.len(),
                                    argument_info.len()
                                ),
                                *span,
                            ));
                        }

                        for (index, (expected, found)) in signature
                            .parameters
                            .iter()
                            .zip(argument_info.iter())
                            .enumerate()
                        {
                            let found_type = &found.0;
                            let found_span = found.1;
                            if !types_compatible(expected, found_type) {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "argument {} has type {}, expected {}",
                                        index + 1,
                                        found_type.describe(),
                                        expected.describe()
                                    ),
                                    found_span,
                                ));
                            }
                        }

                        (*signature.return_type).clone()
                    }
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.errors.push(SemanticError::new(
                            format!("cannot call expression of type {}", other.describe()),
                            expr_span(callee.as_ref()),
                        ));
                        Type::Unknown
                    }
                }
            }
            Expr::Grouping { expression, .. } => self.analyze_expr(expression),
            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let object_type = self.analyze_expr(object);
                match &object_type {
                    Type::Struct(struct_name, fields) => {
                        let resolved_fields = if let Some(Type::Struct(_, stored_fields)) =
                            self.struct_registry.get(struct_name).cloned()
                        {
                            stored_fields
                        } else {
                            fields.clone()
                        };

                        if let Some(field_type) = resolved_fields.get(field) {
                            field_type.clone()
                        } else {
                            self.errors.push(SemanticError::new(
                                format!("struct has no field named '{}'", field),
                                *span,
                            ));
                            Type::Unknown
                        }
                    }
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.errors.push(SemanticError::new(
                            format!(
                                "cannot access field '{}' on non-struct type {}",
                                field,
                                object_type.describe()
                            ),
                            *span,
                        ));
                        Type::Unknown
                    }
                }
            }
            Expr::StructLiteral {
                name,
                fields: field_inits,
                span,
            } => {
                // Look up struct type from the registry
                match self.struct_registry.get(name).cloned() {
                    Some(Type::Struct(struct_name, expected_fields)) => {
                        let mut provided_fields = std::collections::HashSet::new();

                        // Check each provided field
                        for field_init in field_inits {
                            if provided_fields.contains(&field_init.name) {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "duplicate field '{}' in struct literal",
                                        field_init.name
                                    ),
                                    field_init.span,
                                ));
                            }
                            provided_fields.insert(field_init.name.clone());

                            match expected_fields.get(&field_init.name) {
                                Some(expected_type) => {
                                    let actual_type = self.analyze_expr(&field_init.value);
                                    if !types_compatible(expected_type, &actual_type) {
                                        self.errors.push(SemanticError::new(
                                            format!(
                                                "field '{}' expects type {}, got {}",
                                                field_init.name,
                                                expected_type.describe(),
                                                actual_type.describe()
                                            ),
                                            field_init.span,
                                        ));
                                    }
                                }
                                None => {
                                    self.errors.push(SemanticError::new(
                                        format!(
                                            "struct '{}' has no field '{}'",
                                            struct_name, field_init.name
                                        ),
                                        field_init.span,
                                    ));
                                }
                            }
                        }

                        // Check for missing fields
                        for field_name in expected_fields.keys() {
                            if !provided_fields.contains(field_name) {
                                self.errors.push(SemanticError::new(
                                    format!("missing field '{}' in struct literal", field_name),
                                    *span,
                                ));
                            }
                        }

                        Type::Struct(struct_name, expected_fields)
                    }
                    Some(other) => {
                        self.errors.push(SemanticError::new(
                            format!(
                                "'{}' is not a struct type, it is {}",
                                name,
                                other.describe()
                            ),
                            *span,
                        ));
                        Type::Unknown
                    }
                    None => {
                        self.errors.push(SemanticError::new(
                            format!("use of undeclared struct '{}'", name),
                            *span,
                        ));
                        Type::Unknown
                    }
                }
            }
            Expr::EnumLiteral {
                enum_path,
                variant,
                kind,
                span,
            } => {
                if enum_path.len() != 1 {
                    self.errors.push(SemanticError::new(
                        format!(
                            "qualified enum paths are not supported yet: {}",
                            enum_path.join("::")
                        ),
                        *span,
                    ));
                    return Type::Unknown;
                }

                let enum_name = &enum_path[0];
                let Some(enum_info) = self.enum_registry.get(enum_name) else {
                    self.errors.push(SemanticError::new(
                        format!("use of undeclared enum '{}'", enum_name),
                        *span,
                    ));
                    return Type::Unknown;
                };

                let Some(variant_info) = enum_info.variants.get(variant) else {
                    self.errors.push(SemanticError::new(
                        format!("enum '{}' has no variant '{}'", enum_name, variant),
                        *span,
                    ));
                    return Type::Unknown;
                };

                let variant_info = variant_info.clone();

                match variant_info {
                    EnumVariantInfo::Unit => match kind {
                        EnumLiteralKind::Unit => {}
                        EnumLiteralKind::Tuple(arguments) => {
                            for argument in arguments {
                                self.analyze_expr(argument);
                            }
                            self.errors.push(SemanticError::new(
                                format!(
                                    "enum variant '{}::{}' does not accept {} argument(s)",
                                    enum_name,
                                    variant,
                                    arguments.len()
                                ),
                                *span,
                            ));
                        }
                        EnumLiteralKind::Struct(fields) => {
                            for field in fields {
                                self.analyze_expr(&field.value);
                            }
                            self.errors.push(SemanticError::new(
                                format!(
                                    "enum variant '{}::{}' does not accept struct fields ({} provided)",
                                    enum_name,
                                    variant,
                                    fields.len()
                                ),
                                *span,
                            ));
                        }
                    },
                    EnumVariantInfo::Tuple(expected_types) => match kind {
                        EnumLiteralKind::Tuple(arguments) => {
                            if expected_types.len() != arguments.len() {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "enum variant '{}::{}' expects {} argument(s), got {}",
                                        enum_name,
                                        variant,
                                        expected_types.len(),
                                        arguments.len()
                                    ),
                                    *span,
                                ));
                            }

                            for (index, argument) in arguments.iter().enumerate() {
                                let actual = self.analyze_expr(argument);
                                if let Some(expected) = expected_types.get(index) {
                                    if !types_compatible(expected, &actual) {
                                        self.errors.push(SemanticError::new(
                                            format!(
                                                "argument {} of '{}::{}' has type {}, expected {}",
                                                index + 1,
                                                enum_name,
                                                variant,
                                                actual.describe(),
                                                expected.describe()
                                            ),
                                            expr_span(argument),
                                        ));
                                    }
                                } else {
                                    self.errors.push(SemanticError::new(
                                        format!(
                                            "enum variant '{}::{}' does not accept argument {}",
                                            enum_name,
                                            variant,
                                            index + 1
                                        ),
                                        expr_span(argument),
                                    ));
                                }
                            }

                            if expected_types.len() > arguments.len() {
                                for missing_index in arguments.len()..expected_types.len() {
                                    self.errors.push(SemanticError::new(
                                        format!(
                                            "missing argument {} for enum variant '{}::{}'",
                                            missing_index + 1,
                                            enum_name,
                                            variant
                                        ),
                                        *span,
                                    ));
                                }
                            }
                        }
                        EnumLiteralKind::Unit => {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "enum variant '{}::{}' expects {} argument(s), but none were provided",
                                    enum_name,
                                    variant,
                                    expected_types.len()
                                ),
                                *span,
                            ));
                        }
                        EnumLiteralKind::Struct(fields) => {
                            for field in fields {
                                self.analyze_expr(&field.value);
                            }
                            self.errors.push(SemanticError::new(
                                format!(
                                    "enum variant '{}::{}' expects {} positional argument(s), got struct literal",
                                    enum_name,
                                    variant,
                                    expected_types.len()
                                ),
                                *span,
                            ));
                        }
                    },
                    EnumVariantInfo::Struct(expected_fields) => match kind {
                        EnumLiteralKind::Struct(field_inits) => {
                            let mut provided_fields = HashSet::new();

                            for field_init in field_inits {
                                if !provided_fields.insert(field_init.name.clone()) {
                                    self.errors.push(SemanticError::new(
                                        format!(
                                            "duplicate field '{}' in enum variant literal",
                                            field_init.name
                                        ),
                                        field_init.span,
                                    ));
                                    continue;
                                }

                                match expected_fields.get(&field_init.name) {
                                    Some(expected_type) => {
                                        let actual_type = self.analyze_expr(&field_init.value);
                                        if !types_compatible(expected_type, &actual_type) {
                                            self.errors.push(SemanticError::new(
                                                format!(
                                                    "field '{}' in '{}::{}' expects type {}, got {}",
                                                    field_init.name,
                                                    enum_name,
                                                    variant,
                                                    expected_type.describe(),
                                                    actual_type.describe()
                                                ),
                                                field_init.span,
                                            ));
                                        }
                                    }
                                    None => {
                                        self.errors.push(SemanticError::new(
                                            format!(
                                                "enum variant '{}::{}' has no field '{}'",
                                                enum_name, variant, field_init.name
                                            ),
                                            field_init.span,
                                        ));
                                    }
                                }
                            }

                            for field_name in expected_fields.keys() {
                                if !provided_fields.contains(field_name) {
                                    self.errors.push(SemanticError::new(
                                        format!(
                                            "missing field '{}' in enum variant literal '{}::{}'",
                                            field_name, enum_name, variant
                                        ),
                                        *span,
                                    ));
                                }
                            }
                        }
                        EnumLiteralKind::Unit => {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "enum variant '{}::{}' expects {} field(s), but none were provided",
                                    enum_name,
                                    variant,
                                    expected_fields.len()
                                ),
                                *span,
                            ));
                        }
                        EnumLiteralKind::Tuple(arguments) => {
                            for argument in arguments {
                                self.analyze_expr(argument);
                            }
                            self.errors.push(SemanticError::new(
                                format!(
                                    "enum variant '{}::{}' expects named fields ({} total), got {} positional argument(s)",
                                    enum_name,
                                    variant,
                                    expected_fields.len(),
                                    arguments.len()
                                ),
                                *span,
                            ));
                        }
                    },
                }

                Type::Enum(enum_name.clone())
            }
            Expr::ArrayLiteral {
                elements,
                span: _span,
            } => {
                if elements.is_empty() {
                    return Type::Array {
                        element: Box::new(Type::Unknown),
                        length: Some(0),
                    };
                }

                let first_type = self.analyze_expr(&elements[0]);
                let mut element_type = first_type.clone();
                let mut mismatched = false;

                for (i, element) in elements.iter().enumerate().skip(1) {
                    let candidate_type = self.analyze_expr(element);

                    if matches!(element_type, Type::Unknown)
                        && !matches!(candidate_type, Type::Unknown)
                    {
                        element_type = candidate_type.clone();
                    }

                    if !types_compatible(&element_type, &candidate_type) {
                        self.errors.push(SemanticError::new(
                            format!(
                                "array element {} has type {}, expected {}",
                                i,
                                candidate_type.describe(),
                                element_type.describe()
                            ),
                            expr_span(element),
                        ));
                        mismatched = true;
                    }
                }

                let length = Some(elements.len());

                if mismatched {
                    Type::Array {
                        element: Box::new(Type::Unknown),
                        length,
                    }
                } else {
                    Type::Array {
                        element: Box::new(element_type),
                        length,
                    }
                }
            }
            Expr::ArrayRepeat { value, count, .. } => {
                let value_type = self.analyze_expr(value);
                let count_type = self.analyze_expr(count);

                if !matches!(count_type, Type::Integer | Type::Unknown) {
                    self.errors.push(SemanticError::new(
                        format!(
                            "array repeat count must be integer, got {}",
                            count_type.describe()
                        ),
                        expr_span(count),
                    ));
                }

                let resolved_length = match extract_array_length(count) {
                    Some(ArrayLength::Exact(value)) => Some(value),
                    Some(ArrayLength::Negative(span)) => {
                        self.errors.push(SemanticError::new(
                            "array repeat count must be non-negative".to_string(),
                            span,
                        ));
                        None
                    }
                    None => None,
                };

                Type::Array {
                    element: Box::new(value_type),
                    length: resolved_length,
                }
            }
            Expr::Index { array, index, span } => {
                let array_type = self.analyze_expr(array);
                let index_type = self.analyze_expr(index);

                // Check index is an integer
                if !matches!(index_type, Type::Integer) {
                    self.errors.push(SemanticError::new(
                        format!("array index must be integer, got {}", index_type.describe()),
                        *span,
                    ));
                }

                match array_type {
                    Type::Array { element, length } => {
                        self.check_index_bounds(length, index, *span);
                        *element
                    }
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.errors.push(SemanticError::new(
                            format!("cannot index into type {}", other.describe()),
                            expr_span(array),
                        ));
                        Type::Unknown
                    }
                }
            }
        }
    }

    fn report_unused(&mut self, frame: ScopeFrame) {
        if frame.kind == ScopeKind::Module {
            return;
        }

        for (name, info) in frame.symbols {
            if info.used || name.starts_with('_') {
                continue;
            }

            match info.kind {
                SymbolKind::Variable => self.errors.push(SemanticError::new(
                    format!("variable '{}' is never used", name),
                    info.span,
                )),
                SymbolKind::Constant => self.errors.push(SemanticError::new(
                    format!("binding '{}' is never used", name),
                    info.span,
                )),
                SymbolKind::Parameter => self.errors.push(SemanticError::new(
                    format!("parameter '{}' is never used", name),
                    info.span,
                )),
                SymbolKind::Function | SymbolKind::ModuleAlias => {}
            }
        }
    }

    fn resolve_type_name(&mut self, type_name: &TypeName) -> Type {
        let raw = type_name.segments.join("::");
        let normalized = raw.to_ascii_lowercase();
        let ty = match normalized.as_str() {
            "void" => Type::Void,
            "bool" => Type::Bool,
            "string" | "str" => Type::String,
            "f32" | "f64" => Type::Float,
            "int" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => Type::Integer,
            _ => {
                // Check if it's a user-defined struct type
                if let Some(struct_type) = self.scopes.resolve(&raw) {
                    if matches!(struct_type, Type::Struct(..) | Type::Enum(..)) {
                        return struct_type;
                    }
                }
                Type::Unknown
            }
        };

        if ty == Type::Unknown {
            self.errors.push(SemanticError::new(
                format!("unknown type '{}'", raw),
                type_name.span,
            ));
        }

        ty
    }
}

struct FunctionContext<'a> {
    name: &'a str,
    span: Span,
    return_type: Type,
    saw_value_return: bool,
    is_async: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Module,
    Function,
    Block,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SymbolKind {
    Function,
    Variable,
    Parameter,
    Constant,
    ModuleAlias,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FunctionType {
    parameters: Vec<Type>,
    return_type: Box<Type>,
}

impl FunctionType {
    fn describe(&self) -> String {
        let params = if self.parameters.is_empty() {
            String::new()
        } else {
            self.parameters
                .iter()
                .map(|ty| ty.describe())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let return_type = self.return_type.describe();
        if params.is_empty() {
            format!("fn() -> {}", return_type)
        } else {
            format!("fn({}) -> {}", params, return_type)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Type {
    Void,
    Bool,
    Integer,
    Float,
    String,
    Function(FunctionType),
    Struct(String, HashMap<String, Type>),
    Enum(String),
    Array {
        element: Box<Type>,
        length: Option<usize>,
    },
    Module(ModuleAliasInfo),
    Future(Box<Type>),
    Unknown,
}

impl Type {
    fn describe(&self) -> String {
        match self {
            Type::Void => "void".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Integer => "integer".to_string(),
            Type::Float => "float".to_string(),
            Type::String => "string".to_string(),
            Type::Function(signature) => signature.describe(),
            Type::Struct(name, _) => format!("struct {}", name),
            Type::Enum(name) => format!("enum {}", name),
            Type::Array { element, length } => {
                let base = element.describe();
                match length {
                    Some(len) => format!("array<{}; {}>", base, len),
                    None => format!("array<{}>", base),
                }
            }
            Type::Module(info) => format!("module alias '{}'", info.alias),
            Type::Future(inner) => format!("future<{}>", inner.describe()),
            Type::Unknown => "unknown".to_string(),
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, Type::Integer | Type::Float)
    }
}

struct SymbolInfo {
    span: Span,
    kind: SymbolKind,
    ty: Type,
    used: bool,
    mutable: bool,
}

struct ScopeFrame {
    kind: ScopeKind,
    symbols: HashMap<String, SymbolInfo>,
}

struct ScopeStack {
    frames: Vec<ScopeFrame>,
}

impl ScopeStack {
    fn new() -> Self {
        Self { frames: Vec::new() }
    }

    fn push(&mut self, kind: ScopeKind) {
        self.frames.push(ScopeFrame {
            kind,
            symbols: HashMap::new(),
        });
    }

    fn pop(&mut self) -> Option<ScopeFrame> {
        self.frames.pop()
    }

    fn define(
        &mut self,
        name: &str,
        span: Span,
        kind: SymbolKind,
        ty: Type,
        mutable: bool,
    ) -> Result<(), Span> {
        let frame = self
            .frames
            .last_mut()
            .expect("scope stack should not be empty when defining a symbol");
        match frame.symbols.entry(name.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(SymbolInfo {
                    span,
                    kind,
                    ty,
                    used: false,
                    mutable,
                });
                Ok(())
            }
            Entry::Occupied(entry) => Err(entry.get().span),
        }
    }

    fn resolve(&mut self, name: &str) -> Option<Type> {
        self.resolve_mut_internal(name, true)
            .map(|info| info.ty.clone())
    }

    fn set_symbol_type(&mut self, name: &str, ty: Type) -> bool {
        if let Some(info) = self.resolve_mut_internal(name, false) {
            info.ty = ty;
            true
        } else {
            false
        }
    }

    fn resolve_mut_internal(&mut self, name: &str, mark_used: bool) -> Option<&mut SymbolInfo> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(info) = frame.symbols.get_mut(name) {
                if mark_used {
                    info.used = true;
                }
                return Some(info);
            }
        }
        None
    }
}

fn literal_type(literal: &Literal) -> Type {
    match literal {
        Literal::Integer(_) => Type::Integer,
        Literal::Float(_) => Type::Float,
        Literal::String(_) => Type::String,
        Literal::Bool(_) => Type::Bool,
    }
}

fn types_compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual {
        return true;
    }
    if matches!(expected, Type::Unknown) || matches!(actual, Type::Unknown) {
        return true;
    }
    if let (Type::Future(expected_inner), Type::Future(actual_inner)) = (expected, actual) {
        return types_compatible(expected_inner, actual_inner);
    }
    if let (
        Type::Array {
            element: expected_elem,
            length: expected_len,
        },
        Type::Array {
            element: actual_elem,
            length: actual_len,
        },
    ) = (expected, actual)
    {
        if let (Some(expected_len), Some(actual_len)) = (expected_len, actual_len) {
            if expected_len != actual_len {
                return false;
            }
        }
        return types_compatible(expected_elem, actual_elem);
    }
    // For structs, compare names only (fields may differ during registration/resolution)
    if let (Type::Struct(expected_name, _), Type::Struct(actual_name, _)) = (expected, actual) {
        return expected_name == actual_name;
    }
    false
}

fn binary_operator_symbol(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Sub => "-",
        BinaryOperator::Mul => "*",
        BinaryOperator::Div => "/",
        BinaryOperator::Mod => "%",
        BinaryOperator::Equal => "==",
        BinaryOperator::NotEqual => "!=",
        BinaryOperator::Greater => ">",
        BinaryOperator::GreaterEqual => ">=",
        BinaryOperator::Less => "<",
        BinaryOperator::LessEqual => "<=",
        BinaryOperator::And => "&&",
        BinaryOperator::Or => "||",
    }
}

fn unary_operator_symbol(operator: UnaryOperator) -> &'static str {
    match operator {
        UnaryOperator::Not => "!",
        UnaryOperator::Negate => "-",
    }
}

fn module_path_key(path: &ModulePath) -> String {
    path.segments.join("::")
}

fn binding_name_from_expr(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Identifier { name, .. } => Some(name.as_str()),
        Expr::Index { array, .. } => binding_name_from_expr(array),
        Expr::FieldAccess { object, .. } => binding_name_from_expr(object),
        _ => None,
    }
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Literal { span, .. }
        | Expr::Identifier { span, .. }
        | Expr::Call { span, .. }
        | Expr::Unary { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Grouping { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::StructLiteral { span, .. }
        | Expr::EnumLiteral { span, .. }
        | Expr::ArrayLiteral { span, .. }
        | Expr::ArrayRepeat { span, .. }
        | Expr::Index { span, .. }
        | Expr::Await { span, .. } => *span,
    }
}

fn pattern_span(pattern: &MatchPattern) -> Span {
    match pattern {
        MatchPattern::Literal { span, .. }
        | MatchPattern::Identifier { span, .. }
        | MatchPattern::EnumVariant { span, .. } => *span,
    }
}

enum ArrayLength {
    Exact(usize),
    Negative(Span),
}

fn extract_array_length(expr: &Expr) -> Option<ArrayLength> {
    let (value, span) = integer_constant(expr)?;
    if value < 0 {
        Some(ArrayLength::Negative(span))
    } else {
        usize::try_from(value).ok().map(ArrayLength::Exact)
    }
}

fn integer_constant(expr: &Expr) -> Option<(i64, Span)> {
    match expr {
        Expr::Literal {
            value: Literal::Integer(value),
            span,
        } => Some((*value, *span)),
        Expr::Unary {
            operator: UnaryOperator::Negate,
            operand,
            span,
        } => integer_constant(operand)
            .and_then(|(value, _)| value.checked_neg().map(|negated| (negated, *span))),
        Expr::Grouping { expression, .. } => integer_constant(expression),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::Lexer, parser::Parser};

    fn analyze_source(source: &str) -> SemanticResult<()> {
        let tokens = Lexer::new(source).tokenize().expect("lex ok");
        let module = Parser::new(&tokens).parse().expect("parse ok");
        analyze(&module)
    }

    fn analyze_modules_from_sources(sources: &[&str]) -> SemanticResult<()> {
        let mut modules = Vec::new();
        for source in sources {
            let tokens = Lexer::new(source).tokenize().expect("lex ok");
            let module = Parser::new(&tokens).parse().expect("parse ok");
            modules.push(module);
        }
        let refs: Vec<&Module> = modules.iter().collect();
        analyze_modules(&refs)
    }

    #[test]
    fn array_repeat_infers_length_and_allows_valid_index() {
        analyze_source("fn main() { var values = [1; 4]; values[3] = 2; }").expect("analysis ok");
    }

    #[test]
    fn array_repeat_rejects_negative_count() {
        let errors = analyze_source("fn main() { let values = [0; -1]; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("must be non-negative")));
    }

    #[test]
    fn array_repeat_requires_integer_count() {
        let errors = analyze_source("fn main() { let values = [0; 1.5]; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("count must be integer")));
    }

    #[test]
    fn async_function_allows_awaiting_async_call() {
        analyze_source(
            "async fn helper(): i32 { return 1; } async fn compute(): i32 { return await helper(); }",
        )
        .expect("analysis ok");
    }

    #[test]
    fn await_requires_async_context() {
        let errors = analyze_source(
            "fn main(): i32 { return await helper(); } async fn helper(): i32 { return 1; }",
        )
        .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("'await' used outside of async function")));
    }

    #[test]
    fn await_requires_future_value() {
        let errors = analyze_source("async fn main(): i32 { return await 1; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expects a future")));
    }

    #[test]
    fn array_index_reports_out_of_bounds_literal() {
        let errors =
            analyze_source("fn main() { let values = [1, 2]; let last = values[2]; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("out of bounds")));
    }

    #[test]
    fn array_index_rejects_negative_literal() {
        let errors = analyze_source("fn main() { let values = [1, 2]; let first = values[-1]; }")
            .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot be negative")));
    }

    #[test]
    fn accepts_function_with_return_value() {
        analyze_source("fn main(): i32 { let x = 1; return x; }").expect("analysis ok");
    }

    #[test]
    fn detects_duplicate_variable_in_same_scope() {
        let errors =
            analyze_source("fn main(): i32 { let x = 1; let x = 2; return x; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("variable 'x' already defined")));
    }

    #[test]
    fn using_binding_respects_type_annotation() {
        let errors = analyze_source(
            "fn open(): i32 { return 1; } fn main(): i32 { using handle: bool = open(); return 0; }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("annotated as bool")));
    }

    #[test]
    fn detects_return_outside_function() {
        let errors = analyze_source("return;").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("outside of function")));
    }

    #[test]
    fn detects_missing_return_value() {
        let errors = analyze_source("fn main(): i32 { return; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("requires a value")));
    }

    #[test]
    fn detects_function_without_return_statement() {
        let errors = analyze_source("fn main(): i32 { let x = 1; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("must return a value")));
    }

    #[test]
    fn detects_return_value_in_void_function() {
        let errors = analyze_source("fn main() { return 1; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot return a value")));
    }

    #[test]
    fn detects_undefined_identifier() {
        let errors = analyze_source("fn main(): i32 { return y; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("undeclared identifier 'y'")));
    }

    #[test]
    fn detects_duplicate_function_definition() {
        let errors =
            analyze_source("fn foo(): void { return; } fn foo(): void { return; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("function 'foo' already defined")));
    }

    #[test]
    fn allows_shadowing_in_inner_block() {
        analyze_source("fn main(): i32 { let x = 1; { let x = x + 1; return x; } }")
            .expect("analysis ok");
    }

    #[test]
    fn reports_unused_variable() {
        let errors = analyze_source("fn main(): i32 { let x = 1; return 0; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("variable 'x' is never used")));
    }

    #[test]
    fn accepts_typed_let_binding() {
        analyze_source("fn main(): i32 { let value: i32 = 1; return value; }")
            .expect("analysis ok");
    }

    #[test]
    fn detects_mismatched_binding_annotation() {
        let errors =
            analyze_source("fn main(): i32 { let value: bool = 1; return 0; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("annotated as bool")));
    }

    #[test]
    fn detects_mismatched_constant_annotation() {
        let errors = analyze_source("pub let FLAG: bool = 1;").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("annotated as bool")));
    }

    #[test]
    fn accepts_typed_constant_annotation() {
        analyze_source("pub let VALUE: i32 = 1;").expect("analysis ok");
    }

    #[test]
    fn accepts_array_literal_with_matching_types() {
        analyze_source("fn main(): i32 { let numbers = [1, 2, 3]; return numbers[0]; }")
            .expect("analysis ok");
    }

    #[test]
    fn reports_array_element_type_mismatch() {
        let errors =
            analyze_source("fn main(): i32 { let mix = [1, true]; return 0; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("array element 1 has type")));
    }

    #[test]
    fn detects_indexing_non_array_type() {
        let errors =
            analyze_source("fn main(): i32 { let value = 1; return value[0]; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot index into type integer")));
    }

    #[test]
    fn allows_underscore_prefixed_variable() {
        analyze_source("fn main(): i32 { let _x = 1; return 0; }").expect("analysis ok");
    }

    #[test]
    fn reports_unused_parameter() {
        let errors = analyze_source("fn main(x: i32): i32 { return 0; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("parameter 'x' is never used")));
    }

    #[test]
    fn entry_point_main_must_not_accept_parameters() {
        let errors = analyze_source("module app.main; fn main(x: i32): i32 { return x; }")
            .expect_err("should error");
        assert!(errors.iter().any(|error| error
            .message
            .contains("entry point 'main' must not accept parameters")));
    }

    #[test]
    fn entry_point_main_must_return_i32() {
        let errors = analyze_source("module app.main; fn main(): bool { return true; }")
            .expect_err("should error");
        assert!(errors
            .iter()
            .any(|error| error.message.contains("entry point 'main' must return i32")));
    }

    #[test]
    fn entry_point_main_must_declare_return_type() {
        let errors =
            analyze_source("module app.main; fn main() { return; }").expect_err("should error");
        assert!(errors.iter().any(|error| error
            .message
            .contains("entry point 'main' must declare a return type")));
    }

    #[test]
    fn detects_return_type_mismatch() {
        let errors = analyze_source("fn main(): i32 { return true; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("return type mismatch")));
    }

    #[test]
    fn detects_numeric_type_mismatch() {
        let errors = analyze_source("fn main(): i32 { let x = 1; let y = x + true; return x; }")
            .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("must be numeric")));
    }

    #[test]
    fn detects_void_parameter_type() {
        let errors = analyze_source("fn main(x: void): i32 { return 0; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot have type void")));
    }

    #[test]
    fn accepts_function_call_with_matching_signature() {
        analyze_source(
            "fn add(a: i32, b: i32): i32 { return a + b; } fn main(): i32 { return add(1, 2); }",
        )
        .expect("analysis ok");
    }

    #[test]
    fn detects_call_with_wrong_argument_count() {
        let errors = analyze_source(
            "fn add(a: i32, b: i32): i32 { return a + b; } fn main(): i32 { return add(1); }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expects 2 argument(s)")));
    }

    #[test]
    fn detects_call_with_wrong_argument_type() {
        let errors = analyze_source(
            "fn add(a: i32, b: i32): i32 { return a + b; } fn main(): i32 { return add(1, true); }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("argument 2 has type bool")));
    }

    #[test]
    fn allows_import_of_public_constant() {
        analyze_modules_from_sources(&[
            "module demo.values; pub let ANSWER = 42;",
            "module demo.app; import demo.values; fn main(): i32 { return ANSWER; }",
        ])
        .expect("analysis ok");
    }

    #[test]
    fn rejects_mutable_public_binding() {
        let errors = analyze_source("pub var counter = 0;").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot be mutable")));
    }

    #[test]
    fn detects_unknown_module_import() {
        let errors =
            analyze_source("module app.main; import util.math; fn main() { return; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("unknown module 'util::math'")));
    }

    #[test]
    fn detects_self_import() {
        let errors =
            analyze_source("module app.core; import app.core; fn main() { return; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("module cannot import itself")));
    }

    #[test]
    fn accepts_cross_module_call() {
        analyze_modules_from_sources(&[
            "module app.math; pub fn add(a: i32, b: i32): i32 { return a + b; }",
            "module app.main; import app.math; fn main(): i32 { return add(1, 2); }",
        ])
        .expect("analysis ok");
    }

    #[test]
    fn imports_public_struct_type() {
        analyze_modules_from_sources(&[
            "module lib.types; pub struct Point { x: i32, y: i32 }",
            "module app.main; import lib.types; fn make(): Point { return Point { x: 1, y: 2 }; }",
        ])
        .expect("analysis ok");
    }

    #[test]
    fn rejects_private_struct_import() {
        let errors = analyze_modules_from_sources(&[
            "module lib.types; struct Point { x: i32, y: i32 }",
            "module app.main; import lib.types; fn make(): i32 { let _p = Point { x: 1, y: 2 }; return 0; }",
        ])
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("use of undeclared struct 'Point'")));
    }

    #[test]
    fn imports_public_enum_type() {
        analyze_modules_from_sources(&[
            "module lib.types; pub enum Flag { On, Off }",
            "module app.main; import lib.types; fn identity(value: Flag): Flag { return value; }",
        ])
        .expect("analysis ok");
    }

    #[test]
    fn reexports_public_struct() {
        analyze_modules_from_sources(&[
            "module lib.core; pub struct Item { value: i32 }",
            "module lib.wrapper; import lib.core; export lib.core::Item;",
            "module app.main; import lib.wrapper; fn make(): Item { return Item { value: 1 }; }",
        ])
        .expect("analysis ok");
    }

    #[test]
    fn detects_import_conflict_with_local_definition() {
        let errors = analyze_modules_from_sources(&[
            "module app.util; pub fn helper(): i32 { return 1; }",
            "module app.main; import app.util; fn helper(): i32 { return 2; }",
        ])
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("imported symbol 'helper' conflicts")));
    }

    #[test]
    fn reexported_function_is_available_to_dependents() {
        analyze_modules_from_sources(&[
            "module app.math; pub fn add(a: i32, b: i32): i32 { return a + b; }",
            "module app.api; import app.math; export app.math::add;",
            "module app.main; import app.api; fn main(): i32 { return add(3, 4); }",
        ])
        .expect("re-export should expose function");
    }

    #[test]
    fn exporting_unknown_module_reports_error() {
        let errors = analyze_source(
            "module app.api; export missing::helpers::symbol; fn main() { return; }",
        )
        .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("cannot export from unknown module 'missing::helpers'")));
    }

    #[test]
    fn exporting_private_symbol_reports_error() {
        let errors = analyze_modules_from_sources(&[
            "module app.util; fn helper(): i32 { return 1; }",
            "module app.api; export app.util::helper;",
        ])
        .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("module 'app::util' has no exported symbols")));
    }

    #[test]
    fn reexport_conflict_reports_error() {
        let errors = analyze_modules_from_sources(&[
            "module app.math; pub fn add(a: i32, b: i32): i32 { return a + b; }",
            "module app.api; import app.math; pub fn add(a: i32, b: i32): i32 { return a + b; } export app.math::add;",
        ])
        .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("exported symbol 'add' already defined")));
    }

    #[test]
    fn private_functions_are_not_imported() {
        let errors = analyze_modules_from_sources(&[
            "module app.math; fn hidden(): i32 { return 42; }",
            "module app.main; import app.math; fn main(): i32 { return hidden(); }",
        ])
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("undeclared identifier 'hidden'")));
    }

    #[test]
    fn allows_assignment_to_mutable_binding() {
        analyze_source("fn main(): i32 { var x = 0; x = x + 1; return x; }").expect("analysis ok");
    }

    #[test]
    fn detects_assignment_to_immutable_binding() {
        let errors = analyze_source("fn main(): i32 { let x = 0; x = 1; return x; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("cannot assign to immutable binding 'x'")));
    }

    #[test]
    fn defer_outside_function_reports_error() {
        let errors = analyze_source("defer { return; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("defer statement outside of function")));
    }

    #[test]
    fn try_catch_introduces_binding_scope() {
        analyze_source("fn main() { try { return; } catch (_err) { return; } }")
            .expect("analysis ok");
    }

    #[test]
    fn detects_assignment_type_mismatch() {
        let errors =
            analyze_source("fn main(): i32 { var flag = true; flag = 1; return 0; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("cannot assign value of type integer to binding 'flag'")));
    }

    #[test]
    fn detects_if_condition_type_mismatch() {
        let errors = analyze_source("fn main(): i32 { if (1) { return 1; } else { return 0; } }")
            .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("condition of 'if' must have type bool")));
    }

    #[test]
    fn cond_requires_boolean_conditions() {
        let errors = analyze_source("fn main(): i32 { cond { 1 => return 1;, } }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("condition of 'if' must have type bool")));
    }

    #[test]
    fn cond_with_else_clause_is_allowed() {
        analyze_source("fn main(): i32 { cond { false => return 0;, else => return 1;, } }")
            .expect("analysis ok");
    }

    #[test]
    fn unless_requires_boolean_condition() {
        let errors =
            analyze_source("fn main(): i32 { unless (1) { return 0; } return 1; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("operator '!' expects a bool operand")));
    }

    #[test]
    fn unless_with_optional_else_is_allowed() {
        analyze_source("fn main(): i32 { unless (false) { return 1; } else { return 0; } }")
            .expect("analysis ok");
    }

    #[test]
    fn detects_while_condition_type_mismatch() {
        let errors = analyze_source(
            "fn main(): i32 { var x = 0; while (1) { x = x + 1; break; } return x; }",
        )
        .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("condition of 'while' must have type bool")));
    }

    #[test]
    fn detects_break_outside_loop() {
        let errors = analyze_source("fn main(): i32 { break; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("break statement outside of loop")));
    }

    #[test]
    fn detects_continue_outside_loop() {
        let errors = analyze_source("fn main(): i32 { continue; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("continue statement outside of loop")));
    }

    #[test]
    fn allows_break_inside_loop() {
        analyze_source(
            "fn main(): i32 { var x = 0; while (x < 10) { x = x + 1; if (x == 5) { break; } } return x; }",
        )
        .expect("analysis ok");
    }

    #[test]
    fn allows_for_loop_with_initializer_and_condition() {
        analyze_source(
            "fn main(): i32 { var sum = 0; for (let i = 0; i < 10; i + 1) { sum = sum + i; } return sum; }",
        )
        .expect("analysis ok");
    }

    #[test]
    fn detects_for_loop_condition_type_mismatch() {
        let errors = analyze_source("fn main(): i32 { for (let i = 0; 42; i + 1) { } return 0; }")
            .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("condition of 'for' must have type bool")));
    }

    #[test]
    fn allows_struct_declaration() {
        analyze_source("struct Point { x: i32, y: i32 }").expect("analysis ok");
    }

    #[test]
    fn detects_duplicate_struct_fields() {
        let errors = analyze_source("struct Point { x: i32, x: f32 }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("duplicate field 'x'")));
    }

    #[test]
    fn detects_void_struct_field() {
        let errors = analyze_source("struct Data { value: void }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("field 'value' cannot have type void")));
    }

    // ===== New Feature Tests =====

    #[test]
    fn accepts_struct_literal_with_all_fields() {
        let result = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): Point { return Point { x: 10, y: 20 }; }",
        );
        if let Err(errors) = &result {
            for error in errors {
                eprintln!("Error: {}", error.message);
            }
        }
        assert!(result.is_ok());
    }

    #[test]
    fn detects_struct_literal_missing_field() {
        let errors = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): Point { return Point { x: 10 }; }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("missing field 'y'")));
    }

    #[test]
    fn detects_struct_literal_unknown_field() {
        let errors = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): Point { return Point { x: 10, y: 20, z: 30 }; }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("has no field 'z'")));
    }

    #[test]
    fn detects_struct_literal_field_type_mismatch() {
        let errors = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): Point { return Point { x: 10, y: true }; }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expects type integer")));
    }

    #[test]
    fn detects_struct_literal_duplicate_field() {
        let errors = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): Point { return Point { x: 10, x: 20, y: 30 }; }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("duplicate field 'x'")));
    }

    #[test]
    fn accepts_field_assignment() {
        let result = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): void { var p = Point { x: 1, y: 2 }; p.x = 10; }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn detects_field_assignment_type_mismatch() {
        let errors = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): void { var p = Point { x: 1, y: 2 }; p.x = true; }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("expects type integer")));
    }

    #[test]
    fn detects_field_assignment_unknown_field() {
        let errors = analyze_source(
            "struct Point { x: i32, y: i32 } fn main(): void { var p = Point { x: 1, y: 2 }; p.z = 10; }",
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("has no field named 'z'")));
    }

    #[test]
    fn detects_field_assignment_on_non_struct() {
        let errors = analyze_source("fn main(): void { var x = 10; x.field = 20; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("cannot assign to field 'field' on non-struct")));
    }

    #[test]
    fn allows_index_assignment() {
        analyze_source("fn main(): void { var data = [1, 2, 3]; data[1] = 42; }")
            .expect("analysis ok");
    }

    #[test]
    fn detects_index_assignment_type_mismatch() {
        let errors = analyze_source("fn main(): void { var data = [1, 2, 3]; data[0] = true; }")
            .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("array element expects type integer")));
    }

    #[test]
    fn detects_index_assignment_on_non_array() {
        let errors =
            analyze_source("fn main(): void { var value = 1; value[0] = 2; }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot assign through index")));
    }

    #[test]
    fn detects_index_assignment_to_immutable_binding() {
        let errors =
            analyze_source("fn main(): void { let data = [1, 2, 3]; data[0] = 5; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("cannot assign to immutable binding 'data'")));
    }

    #[test]
    fn accepts_logical_and_operator() {
        let result = analyze_source("fn main(): bool { return true && false; }");
        assert!(result.is_ok());
    }

    #[test]
    fn accepts_logical_or_operator() {
        let result = analyze_source("fn main(): bool { return true || false; }");
        assert!(result.is_ok());
    }

    #[test]
    fn detects_logical_and_with_non_bool_left() {
        let errors = analyze_source("fn main(): bool { return 1 && true; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("left operand of '&&' must be boolean")));
    }

    #[test]
    fn detects_logical_and_with_non_bool_right() {
        let errors = analyze_source("fn main(): bool { return true && 1; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("right operand of '&&' must be boolean")));
    }

    #[test]
    fn detects_logical_or_with_non_bool_left() {
        let errors = analyze_source("fn main(): bool { return 1 || true; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("left operand of '||' must be boolean")));
    }

    #[test]
    fn detects_logical_or_with_non_bool_right() {
        let errors = analyze_source("fn main(): bool { return true || 1; }").unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("right operand of '||' must be boolean")));
    }

    #[test]
    fn detects_array_index_non_integer() {
        let errors = analyze_source("fn main(): void { var arr = [1, 2, 3]; var x = arr[true]; }")
            .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("array index must be integer")));
    }

    #[test]
    fn enum_unit_variants() {
        let result = analyze_source("enum Color { Red, Green, Blue } fn main(): void { }");
        assert!(result.is_ok());
    }

    #[test]
    fn enum_tuple_variants() {
        let result = analyze_source(
            "enum Point { TwoD(i32, i32), ThreeD(i32, i32, i32) } fn main(): void { }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn enum_struct_variants() {
        let result = analyze_source(
            "enum Shape { Circle { radius: f32 }, Rectangle { width: f32, height: f32 } } fn main(): void { }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn enum_mixed_variants() {
        let result = analyze_source(
            "enum Message { Quit, Move { x: i32, y: i32 }, Write(string), ChangeColor(i32, i32, i32) } fn main(): void { }",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn detects_duplicate_enum_variants() {
        let errors =
            analyze_source("enum Color { Red, Green, Red } fn main(): void { }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("duplicate variant 'Red'")));
    }

    #[test]
    fn detects_void_in_enum_tuple_variant() {
        let errors = analyze_source("enum Bad { Variant(void) } fn main(): void { }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot contain type void")));
    }

    #[test]
    fn detects_void_in_enum_struct_variant() {
        let errors =
            analyze_source("enum Bad { Variant { field: void } } fn main(): void { }").unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("cannot have type void")));
    }

    #[test]
    fn detects_duplicate_fields_in_enum_struct_variant() {
        let errors = analyze_source("enum Bad { Variant { x: i32, x: i32 } } fn main(): void { }")
            .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("duplicate field 'x'")));
    }

    #[test]
    fn public_enum() {
        let result = analyze_source("pub enum Color { Red, Green, Blue } fn main(): void { }");
        assert!(result.is_ok());
    }

    #[test]
    fn match_accepts_enum_variant_patterns() {
        analyze_source(
            "enum Flag { On, Off } fn react(flag: Flag) { match flag { Flag::On => { return; }, Flag::Off => { return; } } }",
        )
        .expect("analysis ok");
    }

    #[test]
    fn match_binds_enum_tuple_payload() {
        analyze_source(
            "enum Maybe { None, Some(i32) } fn extract(value: Maybe): i32 { match value { Maybe::Some(v) => { return v; }, Maybe::None => { return 0; } } }",
        )
        .expect("analysis ok");
    }

    #[test]
    fn match_struct_variant_binding() {
        analyze_source(
            "enum Message { None, Data { value: i32 } } fn handle(message: Message): i32 { match message { Message::Data { value } => { return value; }, Message::None => { return 0; } } }",
        )
        .expect("analysis ok");
    }

    #[test]
    fn match_rejects_pattern_type_mismatch() {
        let errors = analyze_source(
            "enum Flag { On } fn react(value: i32) { match value { Flag::On => { return; } } }",
        )
        .unwrap_err();
        assert!(errors.iter().any(|error| error.message.contains(
            "match pattern uses enum variant 'Flag::On' but expression has type integer"
        )));
    }

    #[test]
    fn match_reports_tuple_variant_arity_mismatch() {
        let errors = analyze_source(
            "enum State { Empty, Value(i32, i32) } fn react(state: State) { match state { State::Value(x) => { return; }, State::Empty => { return; } } }",
        )
        .unwrap_err();
        assert!(errors.iter().any(|error| error
            .message
            .contains("pattern expects 2 argument(s), got 1")));
    }

    #[test]
    fn match_reports_missing_struct_fields() {
        let errors = analyze_source(
            "enum Message { Data { value: i32 } } fn react(message: Message) { match message { Message::Data {} => { return; } } }",
        )
        .unwrap_err();
        assert!(errors.iter().any(|error| {
            let message = &error.message;
            message.contains("pattern expects named fields, but none were provided")
                || message.contains("missing field 'value' in pattern for enum variant")
        }));
    }
}
