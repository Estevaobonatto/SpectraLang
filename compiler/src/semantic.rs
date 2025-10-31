use std::collections::{hash_map::Entry, HashMap, HashSet};

use crate::ast::{
    BinaryOperator, Block, Constant, Export, Expr, Function, Import, Item, Literal, Module,
    ModulePath, Stmt, TypeName, UnaryOperator, Visibility,
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
struct EnumExport {
    ty: Type,
    span: Span,
}

#[derive(Clone, Default)]
struct ModuleExport {
    functions: HashMap<String, FunctionExport>,
    constants: HashMap<String, ConstantExport>,
    structs: HashMap<String, StructExport>,
    enums: HashMap<String, EnumExport>,
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
    current_module_imports: HashSet<String>,
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
            current_module_imports: HashSet::new(),
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
                            return_type: Box::new(if function.return_type.is_some() {
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
                    Item::Constant(constant) => {
                        if constant.visibility != Visibility::Public {
                            continue;
                        }
                        if constant.mutable {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "exported binding '{}' must be declared with 'let', mutable exports are not supported",
                                    constant.name
                                ),
                                constant.span,
                            ));
                            continue;
                        }
                        let exports = self.module_exports.entry(key.clone()).or_default();
                        if exports.functions.contains_key(&constant.name)
                            || exports.structs.contains_key(&constant.name)
                            || exports.enums.contains_key(&constant.name)
                        {
                            let previous_location = exports
                                .functions
                                .get(&constant.name)
                                .map(|entry| entry.span.start_location)
                                .or_else(|| {
                                    exports
                                        .structs
                                        .get(&constant.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .or_else(|| {
                                    exports
                                        .enums
                                        .get(&constant.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .unwrap_or(constant.span.start_location);
                            self.errors.push(SemanticError::new(
                                format!(
                                    "constant '{}' conflicts with existing exported symbol in module '{}' (previous definition at {})",
                                    constant.name, key, previous_location
                                ),
                                constant.span,
                            ));
                            continue;
                        }
                        match exports.constants.entry(constant.name.clone()) {
                            Entry::Vacant(entry) => {
                                entry.insert(ConstantExport {
                                    ty: Type::Unknown,
                                    span: constant.span,
                                });
                            }
                            Entry::Occupied(entry) => {
                                let previous_location = entry.get().span.start_location;
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "constant '{}' already defined in module '{}' (previous definition at {})",
                                        constant.name, key, previous_location
                                    ),
                                    constant.span,
                                ));
                            }
                        }
                    }
                    Item::Struct(struct_decl) => {
                        if struct_decl.visibility != Visibility::Public {
                            continue;
                        }
                        let exports = self.module_exports.entry(key.clone()).or_default();
                        if exports.functions.contains_key(&struct_decl.name)
                            || exports.constants.contains_key(&struct_decl.name)
                            || exports.enums.contains_key(&struct_decl.name)
                        {
                            let previous_location = exports
                                .functions
                                .get(&struct_decl.name)
                                .map(|entry| entry.span.start_location)
                                .or_else(|| {
                                    exports
                                        .constants
                                        .get(&struct_decl.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .or_else(|| {
                                    exports
                                        .enums
                                        .get(&struct_decl.name)
                                        .map(|entry| entry.span.start_location)
                                })
                                .unwrap_or(struct_decl.span.start_location);
                            self.errors.push(SemanticError::new(
                                format!(
                                    "struct '{}' conflicts with existing exported symbol in module '{}' (previous definition at {})",
                                    struct_decl.name, key, previous_location
                                ),
                                struct_decl.span,
                            ));
                            continue;
                        }
                        match exports.structs.entry(struct_decl.name.clone()) {
                            Entry::Vacant(entry) => {
                                entry.insert(StructExport {
                                    ty: Type::Struct(struct_decl.name.clone(), HashMap::new()),
                                    span: struct_decl.span,
                                });
                            }
                            Entry::Occupied(entry) => {
                                let previous_location = entry.get().span.start_location;
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "struct '{}' already defined in module '{}' (previous definition at {})",
                                        struct_decl.name, key, previous_location
                                    ),
                                    struct_decl.span,
                                ));
                            }
                        }
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

        let (function_export, constant_export, struct_export, enum_export) =
            match self.module_exports.get(&target_key) {
                Some(exports) => (
                    exports.functions.get(&export.symbol).cloned(),
                    exports.constants.get(&export.symbol).cloned(),
                    exports.structs.get(&export.symbol).cloned(),
                    exports.enums.get(&export.symbol).cloned(),
                ),
                None => {
                    self.errors.push(SemanticError::new(
                        format!("module '{}' has no exported symbols", target_key),
                        export.module_path.span,
                    ));
                    return;
                }
            };

        let exports = self
            .module_exports
            .entry(current_key.to_string())
            .or_default();

        if let Some(function_export) = function_export {
            if exports.constants.contains_key(&export.symbol)
                || exports.structs.contains_key(&export.symbol)
                || exports.enums.contains_key(&export.symbol)
            {
                let previous_location = exports
                    .constants
                    .get(&export.symbol)
                    .map(|entry| entry.span.start_location)
                    .or_else(|| {
                        exports
                            .structs
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .or_else(|| {
                        exports
                            .enums
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .unwrap_or(export.symbol_span.start_location);
                self.errors.push(SemanticError::new(
                    format!(
                        "exported symbol '{}' conflicts with existing symbol in module '{}' (previous definition at {})",
                        export.symbol, current_key, previous_location
                    ),
                    export.symbol_span,
                ));
                return;
            }

            match exports.functions.entry(export.symbol.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(FunctionExport {
                        signature: function_export.signature.clone(),
                        span: export.symbol_span,
                    });
                }
                Entry::Occupied(entry) => {
                    let previous_location = entry.get().span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "exported symbol '{}' already defined in module '{}' (previous definition at {})",
                            export.symbol, current_key, previous_location
                        ),
                        export.symbol_span,
                    ));
                }
            }
            return;
        }

        if let Some(constant_export) = constant_export {
            if exports.functions.contains_key(&export.symbol)
                || exports.structs.contains_key(&export.symbol)
                || exports.enums.contains_key(&export.symbol)
            {
                let previous_location = exports
                    .functions
                    .get(&export.symbol)
                    .map(|entry| entry.span.start_location)
                    .or_else(|| {
                        exports
                            .structs
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .or_else(|| {
                        exports
                            .enums
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .unwrap_or(export.symbol_span.start_location);
                self.errors.push(SemanticError::new(
                    format!(
                        "exported symbol '{}' conflicts with existing symbol in module '{}' (previous definition at {})",
                        export.symbol, current_key, previous_location
                    ),
                    export.symbol_span,
                ));
                return;
            }

            match exports.constants.entry(export.symbol.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(ConstantExport {
                        ty: constant_export.ty.clone(),
                        span: export.symbol_span,
                    });
                }
                Entry::Occupied(entry) => {
                    let previous_location = entry.get().span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "exported symbol '{}' already defined in module '{}' (previous definition at {})",
                            export.symbol, current_key, previous_location
                        ),
                        export.symbol_span,
                    ));
                }
            }
            return;
        }

        if let Some(struct_export) = struct_export {
            if exports.functions.contains_key(&export.symbol)
                || exports.constants.contains_key(&export.symbol)
                || exports.enums.contains_key(&export.symbol)
            {
                let previous_location = exports
                    .functions
                    .get(&export.symbol)
                    .map(|entry| entry.span.start_location)
                    .or_else(|| {
                        exports
                            .constants
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .or_else(|| {
                        exports
                            .enums
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .unwrap_or(export.symbol_span.start_location);
                self.errors.push(SemanticError::new(
                    format!(
                        "exported symbol '{}' conflicts with existing symbol in module '{}' (previous definition at {})",
                        export.symbol, current_key, previous_location
                    ),
                    export.symbol_span,
                ));
                return;
            }

            match exports.structs.entry(export.symbol.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(struct_export);
                }
                Entry::Occupied(entry) => {
                    let previous_location = entry.get().span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "exported symbol '{}' already defined in module '{}' (previous definition at {})",
                            export.symbol, current_key, previous_location
                        ),
                        export.symbol_span,
                    ));
                }
            }
            return;
        }

        if let Some(enum_export) = enum_export {
            if exports.functions.contains_key(&export.symbol)
                || exports.constants.contains_key(&export.symbol)
                || exports.structs.contains_key(&export.symbol)
            {
                let previous_location = exports
                    .functions
                    .get(&export.symbol)
                    .map(|entry| entry.span.start_location)
                    .or_else(|| {
                        exports
                            .constants
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .or_else(|| {
                        exports
                            .structs
                            .get(&export.symbol)
                            .map(|entry| entry.span.start_location)
                    })
                    .unwrap_or(export.symbol_span.start_location);
                self.errors.push(SemanticError::new(
                    format!(
                        "exported symbol '{}' conflicts with existing symbol in module '{}' (previous definition at {})",
                        export.symbol, current_key, previous_location
                    ),
                    export.symbol_span,
                ));
                return;
            }

            match exports.enums.entry(export.symbol.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(enum_export);
                }
                Entry::Occupied(entry) => {
                    let previous_location = entry.get().span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "exported symbol '{}' already defined in module '{}' (previous definition at {})",
                            export.symbol, current_key, previous_location
                        ),
                        export.symbol_span,
                    ));
                }
            }
            return;
        }

        self.errors.push(SemanticError::new(
            format!(
                "module '{}' does not export symbol '{}'",
                target_key, export.symbol
            ),
            export.symbol_span,
        ));
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

        if let Some(frame) = self.scopes.pop() {
            self.report_unused(frame);
        }

        self.current_module_key = None;
        self.current_module_signatures.clear();
    }

    fn register_structs(&mut self, module: &'a Module) {
        for item in &module.items {
            if let Item::Struct(struct_decl) = item {
                // Register struct with empty fields initially
                // Fields will be validated later in analyze_struct
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
                // Register enum name initially
                // Variants will be validated later in analyze_enum
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

    fn introduce_imports(&mut self, module: &'a Module) {
        let mut seen_modules = HashSet::new();
        for item in &module.items {
            if let Item::Import(import) = item {
                let key = module_path_key(&import.path);

                if !seen_modules.insert(key.clone()) {
                    continue;
                }

                if self.current_module_key.as_deref() == Some(key.as_str()) {
                    continue;
                }

                let (function_exports, constant_exports, struct_exports, enum_exports) =
                    match self.module_exports.get(&key) {
                        Some(exports) => (
                            exports
                                .functions
                                .iter()
                                .map(|(name, export)| (name.clone(), export.clone()))
                                .collect::<Vec<_>>(),
                            exports
                                .constants
                                .iter()
                                .map(|(name, export)| (name.clone(), export.clone()))
                                .collect::<Vec<_>>(),
                            exports
                                .structs
                                .iter()
                                .map(|(name, export)| (name.clone(), export.clone()))
                                .collect::<Vec<_>>(),
                            exports
                                .enums
                                .iter()
                                .map(|(name, export)| (name.clone(), export.clone()))
                                .collect::<Vec<_>>(),
                        ),
                        None => continue,
                    };

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
                        function_updates.push((
                            name.clone(),
                            export.signature.clone(),
                            export.span,
                        ));
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
        }
    }

    fn analyze_function(&mut self, function: &'a Function) {
        self.scopes.push(ScopeKind::Function);
        let signature = self
            .current_module_signatures
            .get(&function.name)
            .cloned()
            .unwrap_or_else(|| self.build_function_signature(function));
        let return_type = (*signature.return_type).clone();
        self.current_function = Some(FunctionContext {
            name: &function.name,
            span: function.span,
            return_type: return_type.clone(),
            saw_value_return: false,
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

        for variant in &enum_decl.variants {
            // Check for duplicate variant names
            if variant_names.contains(&variant.name) {
                self.errors.push(SemanticError::new(
                    format!(
                        "duplicate variant '{}' in enum '{}'",
                        variant.name, enum_decl.name
                    ),
                    variant.span,
                ));
            } else {
                variant_names.insert(variant.name.clone());
            }

            // Validate variant data types
            if let Some(ref data) = variant.data {
                match data {
                    crate::ast::EnumVariantData::Tuple(types) => {
                        for ty in types {
                            let resolved_type = self.resolve_type_name(ty);
                            if resolved_type == Type::Void {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "enum variant '{}' cannot contain type void",
                                        variant.name
                                    ),
                                    variant.span,
                                ));
                            }
                        }
                    }
                    crate::ast::EnumVariantData::Struct(fields) => {
                        let mut field_names = HashSet::new();
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

                            if field_names.contains(&field.name) {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "duplicate field '{}' in variant '{}' of enum '{}'",
                                        field.name, variant.name, enum_decl.name
                                    ),
                                    field.span,
                                ));
                            } else {
                                field_names.insert(field.name.clone());
                            }
                        }
                    }
                }
            }
        }

        // The enum type was already registered in register_enums

        if let Some(module_key) = self.current_module_key.as_ref() {
            if let Some(exports) = self.module_exports.get_mut(module_key) {
                if let Some(entry) = exports.enums.get_mut(&enum_decl.name) {
                    entry.ty = Type::Enum(enum_decl.name.clone());
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
        let return_type = function
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_name(ty))
            .unwrap_or(Type::Void);
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
                    match &arm.pattern {
                        crate::ast::MatchPattern::Literal { value, span } => {
                            let pattern_type = literal_type(value);
                            if !types_compatible(&expr_type, &pattern_type) {
                                self.errors.push(SemanticError::new(
                                    format!(
                                        "match pattern type {} does not match expression type {}",
                                        pattern_type.describe(),
                                        expr_type.describe()
                                    ),
                                    *span,
                                ));
                            }
                        }
                        crate::ast::MatchPattern::Identifier { .. } => {
                            // Pattern binding - could be validated more thoroughly in the future
                        }
                    }
                    self.analyze_statement(&arm.body);
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

    fn analyze_return(&mut self, value: Option<&Expr>, span: Span) {
        if let Some(mut context) = self.current_function.take() {
            if context.return_type == Type::Void {
                if let Some(expr) = value {
                    let expr_type = self.analyze_expr(expr);
                    self.errors.push(SemanticError::new(
                        format!(
                            "return statement in function '{}' cannot return a value (found {})",
                            context.name,
                            expr_type.describe()
                        ),
                        expr_span(expr),
                    ));
                }
            } else {
                match value {
                    Some(expr) => {
                        let expr_type = self.analyze_expr(expr);
                        if !types_compatible(&context.return_type, &expr_type) {
                            self.errors.push(SemanticError::new(
                                format!(
                                    "return type mismatch in function '{}': expected {}, found {}",
                                    context.name,
                                    context.return_type.describe(),
                                    expr_type.describe()
                                ),
                                expr_span(expr),
                            ));
                        }
                        context.saw_value_return = true;
                    }
                    None => {
                        self.errors.push(SemanticError::new(
                            format!(
                                "return statement in function '{}' requires a value of type {}",
                                context.name,
                                context.return_type.describe()
                            ),
                            span,
                        ));
                    }
                }
            }
            self.current_function = Some(context);
        } else {
            if let Some(expr) = value {
                self.analyze_expr(expr);
            }
            self.errors.push(SemanticError::new(
                "return statement outside of function",
                span,
            ));
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
            Expr::ArrayLiteral {
                elements,
                span: _span,
            } => {
                if elements.is_empty() {
                    // Empty array - element type is unknown for now
                    return Type::Array(Box::new(Type::Unknown));
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

                if mismatched {
                    Type::Array(Box::new(Type::Unknown))
                } else {
                    Type::Array(Box::new(element_type))
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
                    Type::Array(element_type) => *element_type,
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
                SymbolKind::Function => {}
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
    Array(Box<Type>),
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
            Type::Array(element) => format!("array<{}>", element.describe()),
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
    if let (Type::Array(expected_elem), Type::Array(actual_elem)) = (expected, actual) {
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
        | Expr::ArrayLiteral { span, .. }
        | Expr::Index { span, .. } => *span,
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
}
