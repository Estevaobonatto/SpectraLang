use std::collections::{hash_map::Entry, HashMap};

use crate::ast::{Block, Expr, Function, Item, Module, Stmt};
use crate::error::{SemanticError, SemanticResult};
use crate::span::Span;

pub fn analyze(module: &Module) -> SemanticResult<()> {
    let mut analyzer = Analyzer::new();
    analyzer.analyze_module(module);
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
}

impl<'a> Analyzer<'a> {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            scopes: ScopeStack::new(),
            current_function: None,
        }
    }

    fn analyze_module(&mut self, module: &'a Module) {
        self.scopes.push(ScopeKind::Module);

        for item in &module.items {
            if let Item::Function(function) = item {
                if let Err(previous_span) =
                    self.scopes
                        .define(&function.name, function.span, SymbolKind::Function)
                {
                    let previous_location = previous_span.start_location;
                    self.errors.push(SemanticError::new(
                        format!(
                            "function '{}' already defined (previous definition at {})",
                            function.name, previous_location
                        ),
                        function.span,
                    ));
                }
            }
        }

        for item in &module.items {
            match item {
                Item::Function(function) => self.analyze_function(function),
                Item::Stmt(statement) => self.analyze_statement(statement),
            }
        }

        if let Some(frame) = self.scopes.pop() {
            self.report_unused(frame);
        }
    }

    fn analyze_function(&mut self, function: &'a Function) {
        self.scopes.push(ScopeKind::Function);
        self.current_function = Some(FunctionContext {
            name: &function.name,
            span: function.span,
            expects_value: function.return_type.is_some(),
            has_return_with_value: false,
        });

        for parameter in &function.parameters {
            if let Err(previous_span) =
                self.scopes
                    .define(&parameter.name, parameter.span, SymbolKind::Parameter)
            {
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
            if context.expects_value && !context.has_return_with_value {
                self.errors.push(SemanticError::new(
                    format!(
                        "function '{}' must return a value in all paths",
                        context.name
                    ),
                    context.span,
                ));
            }
        }

        if let Some(frame) = self.scopes.pop() {
            self.report_unused(frame);
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
                self.analyze_expr(value);
                if let Err(previous_span) = self.scopes.define(name, *span, SymbolKind::Variable) {
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
            Stmt::Expr(expr) => self.analyze_expr(expr),
            Stmt::Return { value, span } => self.analyze_return(value.as_ref(), *span),
            Stmt::Block(block) => self.analyze_block(block),
        }
    }

    fn analyze_return(&mut self, value: Option<&Expr>, span: Span) {
        if let Some(mut context) = self.current_function.take() {
            if context.expects_value {
                if let Some(expr) = value {
                    self.analyze_expr(expr);
                    context.has_return_with_value = true;
                } else {
                    self.errors.push(SemanticError::new(
                        format!(
                            "return statement in function '{}' requires a value",
                            context.name
                        ),
                        span,
                    ));
                }
            } else if let Some(expr) = value {
                self.analyze_expr(expr);
                self.errors.push(SemanticError::new(
                    format!(
                        "return statement in function '{}' cannot return a value",
                        context.name
                    ),
                    span,
                ));
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

    fn analyze_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal { .. } => {}
            Expr::Identifier { name, span } => {
                if self.scopes.resolve(name).is_none() {
                    self.errors.push(SemanticError::new(
                        format!("use of undeclared identifier '{}'", name),
                        *span,
                    ));
                }
            }
            Expr::Unary { operand, .. } => self.analyze_expr(operand),
            Expr::Binary { left, right, .. } => {
                self.analyze_expr(left);
                self.analyze_expr(right);
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
}

struct FunctionContext<'a> {
    name: &'a str,
    span: Span,
    expects_value: bool,
    has_return_with_value: bool,
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

struct SymbolInfo {
    span: Span,
    kind: SymbolKind,
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

    fn define(&mut self, name: &str, span: Span, kind: SymbolKind) -> Result<(), Span> {
        let frame = self
            .frames
            .last_mut()
            .expect("scope stack should not be empty when defining a symbol");
        match frame.symbols.entry(name.to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(SymbolInfo {
                    span,
                    kind,
                    used: false,
                });
                Ok(())
            }
            Entry::Occupied(entry) => Err(entry.get().span),
        }
    }

    fn resolve(&mut self, name: &str) -> Option<Span> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(info) = frame.symbols.get_mut(name) {
                info.used = true;
                return Some(info.span);
            }
        }
        None
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
}
