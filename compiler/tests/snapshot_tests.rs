use spectra_compiler::ast::{
    Expression, ExpressionKind, Item, Module, Pattern, Statement, StatementKind, TypeAnnotation,
    TypeAnnotationKind,
};
use spectra_compiler::{CompilationOptions, CompilationPipeline, CompilerError, Lexer, Parser};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn parse(source: &str) -> Module {
    let tokens = Lexer::new(source).tokenize().expect("lexing should pass");
    Parser::new(tokens, HashSet::new())
        .parse()
        .expect("parsing should pass")
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(name);
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read snapshot {}: {err}", path.display()));
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "snapshot {name} changed"
    );
}

fn type_annotation(ty: &Option<TypeAnnotation>) -> String {
    ty.as_ref()
        .map(type_annotation_inner)
        .unwrap_or_else(|| "unit".to_string())
}

fn type_annotation_inner(ty: &TypeAnnotation) -> String {
    match &ty.kind {
        TypeAnnotationKind::Simple { segments } => segments.join("."),
        TypeAnnotationKind::Tuple { elements } => {
            let elems = elements
                .iter()
                .map(type_annotation_inner)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({elems})")
        }
        TypeAnnotationKind::Function {
            params,
            return_type,
        } => {
            let params = params
                .iter()
                .map(type_annotation_inner)
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({params}) -> {}", type_annotation_inner(return_type))
        }
        TypeAnnotationKind::Generic { name, type_args } => {
            let args = type_args
                .iter()
                .map(type_annotation_inner)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}<{args}>")
        }
        TypeAnnotationKind::DynTrait { trait_name } => format!("dyn {trait_name}"),
    }
}

fn expression(expr: &Expression) -> String {
    match &expr.kind {
        ExpressionKind::Identifier(name) => format!("ident({name})"),
        ExpressionKind::NumberLiteral(value) => format!("number({value})"),
        ExpressionKind::BoolLiteral(value) => format!("bool({value})"),
        ExpressionKind::StringLiteral(value) => format!("string({value:?})"),
        ExpressionKind::Binary {
            left,
            operator,
            right,
        } => format!(
            "binary({} {:?} {})",
            expression(left),
            operator,
            expression(right)
        ),
        ExpressionKind::Call { callee, arguments } => {
            let args = arguments
                .iter()
                .map(expression)
                .collect::<Vec<_>>()
                .join(", ");
            format!("call({} [{}])", expression(callee), args)
        }
        ExpressionKind::AsyncBlock(block) => {
            let body = block
                .statements
                .iter()
                .map(statement)
                .collect::<Vec<_>>()
                .join("; ");
            format!("async_block({body})")
        }
        ExpressionKind::Lambda {
            is_async,
            params,
            body,
        } => {
            let params = params
                .iter()
                .map(|param| param.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let prefix = if *is_async { "async_lambda" } else { "lambda" };
            format!("{prefix}([{params}] {})", expression(body))
        }
        other => format!("{other:?}"),
    }
}

fn pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Identifier(name) => format!("bind({name})"),
        Pattern::Wildcard => "_".to_string(),
        other => format!("{other:?}"),
    }
}

fn statement(stmt: &Statement) -> String {
    match &stmt.kind {
        StatementKind::Let(let_stmt) => {
            let value = let_stmt
                .value
                .as_ref()
                .map(expression)
                .unwrap_or_else(|| "none".to_string());
            format!(
                "let {}: {} = {value}",
                pattern(&let_stmt.pattern),
                type_annotation(&let_stmt.ty)
            )
        }
        StatementKind::Return(ret) => ret
            .value
            .as_ref()
            .map(|value| format!("return {}", expression(value)))
            .unwrap_or_else(|| "return".to_string()),
        StatementKind::Expression(expr) => format!("expr {}", expression(expr)),
        other => format!("{other:?}"),
    }
}

fn ast_snapshot(module: &Module) -> String {
    let mut out = String::new();
    out.push_str(&format!("module {}\n", module.name));
    for item in &module.items {
        match item {
            Item::Import(import) => out.push_str(&format!(
                "import path={} alias={} names={} reexport={}\n",
                import.path.join("."),
                import.alias.as_deref().unwrap_or("-"),
                import
                    .names
                    .as_ref()
                    .map(|names| names.join(","))
                    .unwrap_or_else(|| "-".to_string()),
                import.is_reexport
            )),
            Item::Function(function) => {
                let params = function
                    .params
                    .iter()
                    .map(|param| {
                        format!(
                            "{}: {}",
                            param.name,
                            param
                                .ty
                                .as_ref()
                                .map(type_annotation_inner)
                                .unwrap_or_else(|| "unknown".to_string())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let prefix = if function.is_async { "async fn" } else { "fn" };
                out.push_str(&format!(
                    "{} {:?} {}({params}) -> {}\n",
                    prefix,
                    function.visibility,
                    function.name,
                    type_annotation(&function.return_type)
                ));
                for stmt in &function.body.statements {
                    out.push_str(&format!("  {}\n", statement(stmt)));
                }
            }
            other => out.push_str(&format!("item {other:?}\n")),
        }
    }
    out
}

#[test]
fn ast_snapshot_covers_parser_stage() {
    let source = r#"
        module snapshot;

        import std.tensor as tensor;

        fn add(lhs: int, rhs: int) -> int {
            let total = lhs + rhs;
            return total;
        }

        async fn fetch() {
            let task = async { 1 };
            let work = async |value: int| value;
        }

        pub fn main() -> int {
            let value = add(40, 2);
            return value;
        }
    "#;
    let module = parse(source);
    assert_snapshot("parser_ast.snap", &ast_snapshot(&module));
}

#[test]
fn diagnostic_snapshot_covers_semantic_stage() {
    let source = r#"
        module diag;

        pub fn main() -> int {
            return missing_symbol;
        }
    "#;
    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    let errors = pipeline
        .compile(source, "diag.spectra")
        .expect_err("semantic diagnostic should be produced");
    let mut lines = Vec::new();
    for error in errors {
        match error {
            CompilerError::Lexical(err) => lines.push(format!(
                "lexical|{}|{}|{}",
                err.code.as_deref().unwrap_or("-"),
                err.message,
                err.hint.as_deref().unwrap_or("-")
            )),
            CompilerError::Parse(err) => lines.push(format!(
                "parse|{}|{}|{}",
                err.code.as_deref().unwrap_or("-"),
                err.message,
                err.hint.as_deref().unwrap_or("-")
            )),
            CompilerError::Semantic(err) => lines.push(format!(
                "semantic|{}|{}|{}",
                err.code.as_deref().unwrap_or("-"),
                err.message,
                err.hint.as_deref().unwrap_or("-")
            )),
            CompilerError::Midend(err) => lines.push(format!("midend|-|{}|-", err.message)),
            CompilerError::Backend(err) => lines.push(format!("backend|-|{}|-", err.message)),
        }
    }
    assert_snapshot(
        "semantic_diagnostic.snap",
        &format!("{}\n", lines.join("\n")),
    );
}
