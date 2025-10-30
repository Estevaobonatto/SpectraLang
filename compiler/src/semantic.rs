use std::collections::{hash_map::Entry, HashMap, HashSet};

use crate::ast::{
    BinaryOperator, Block, Export, Expr, Function, Import, Item, Literal, Module, ModulePath, Stmt,
    TypeName, UnaryOperator, Visibility,
};
use crate::error::{SemanticError, SemanticResult};
use crate::span::Span;

#[derive(Clone)]
struct FunctionExport {
    signature: FunctionType,
    span: Span,
}

#[derive(Clone, Default)]
struct ModuleExport {
    functions: HashMap<String, FunctionExport>,
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
                if let Item::Function(function) = item {
                    if function.visibility != Visibility::Public {
                        continue;
                    }
                    let signature = self.build_function_signature(function);
                    let exports = self
                        .module_exports
                        .entry(key.clone())
                        .or_insert_with(ModuleExport::default);
                    match exports.functions.entry(function.name.clone()) {
                        Entry::Vacant(entry) => {
                            entry.insert(FunctionExport {
                                signature: signature.clone(),
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

        let Some(signature) = self
            .module_exports
            .get(&target_key)
            .and_then(|exports| exports.functions.get(&export.symbol))
            .map(|exported| exported.signature.clone())
        else {
            self.errors.push(SemanticError::new(
                format!(
                    "module '{}' does not export function '{}'",
                    target_key, export.symbol
                ),
                export.symbol_span,
            ));
            return;
        };

        let exports = self
            .module_exports
            .entry(current_key.to_string())
            .or_insert_with(ModuleExport::default);

        match exports.functions.entry(export.symbol.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(FunctionExport {
                    signature,
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
    }

    fn analyze_module(&mut self, module: &'a Module) {
        self.scopes.push(ScopeKind::Module);
        self.current_module_key = module.name.as_ref().map(|path| module_path_key(path));
        self.current_module_signatures.clear();
        self.register_functions(module);
        self.validate_imports(module);
        self.introduce_imports(module);

        for item in &module.items {
            match item {
                Item::Function(function) => self.analyze_function(function),
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

    fn register_functions(&mut self, module: &'a Module) {
        for item in &module.items {
            if let Item::Function(function) = item {
                let signature = self.signature_for_function(module, function);
                if let Err(previous_span) = self.scopes.define(
                    &function.name,
                    function.span,
                    SymbolKind::Function,
                    Type::Function(signature.clone()),
                ) {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "function '{}' already defined (previous definition at {})",
                            function.name, previous_location
                        ),
                        function.span,
                    ));
                } else {
                    self.current_module_signatures
                        .insert(function.name.clone(), signature);
                }
            }
        }
    }

    fn signature_for_function(
        &mut self,
        module: &'a Module,
        function: &'a Function,
    ) -> FunctionType {
        if let Some(key) = module.name.as_ref().map(|path| module_path_key(path)) {
            if let Some(exports) = self.module_exports.get(&key) {
                if let Some(export) = exports.functions.get(&function.name) {
                    return export.signature.clone();
                }
            }
        }
        self.build_function_signature(function)
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

                let Some(exports) = self.module_exports.get(&key) else {
                    continue;
                };

                for (name, export) in &exports.functions {
                    if let Err(previous_span) = self.scopes.define(
                        name,
                        export.span,
                        SymbolKind::Function,
                        Type::Function(export.signature.clone()),
                    ) {
                        let previous_location = previous_span.start_location;
                        self.errors.push(SemanticError::new(
                            format!(
                                "imported symbol '{}' conflicts with existing definition (previous definition at {})",
                                name, previous_location
                            ),
                            import.span,
                        ));
                    }
                }
            }
        }
    }

    fn validate_imports(&mut self, module: &'a Module) {
        let current_key = module.name.as_ref().map(|path| module_path_key(path));
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
                name, value, span, ..
            } => {
                let value_type = self.analyze_expr(value);
                if let Err(previous_span) =
                    self.scopes
                        .define(name, *span, SymbolKind::Variable, value_type)
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
            Stmt::Expr(expr) => {
                self.analyze_expr(expr);
            }
            Stmt::Return { value, span } => self.analyze_return(value.as_ref(), *span),
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
            _ => Type::Unknown,
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

    fn define(&mut self, name: &str, span: Span, kind: SymbolKind, ty: Type) -> Result<(), Span> {
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
                });
                Ok(())
            }
            Entry::Occupied(entry) => Err(entry.get().span),
        }
    }

    fn resolve(&mut self, name: &str) -> Option<Type> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(info) = frame.symbols.get_mut(name) {
                info.used = true;
                return Some(info.ty.clone());
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
    expected == actual || matches!(expected, Type::Unknown) || matches!(actual, Type::Unknown)
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
        | Expr::Grouping { span, .. } => *span,
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
        let errors =
            analyze_source("module app.api; export missing::helpers::symbol; fn main() { return; }")
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
            .contains("module 'app::util' does not export function 'helper'")));
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
}
