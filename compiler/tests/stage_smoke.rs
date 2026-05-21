use spectra_compiler::{
    analyze_modules, CompilationOptions, CompilationPipeline, CompilerError, Lexer, Parser,
};
use std::collections::HashSet;

fn parse_module(source: &str) -> spectra_compiler::Module {
    let tokens = Lexer::new(source).tokenize().expect("lexer should succeed");
    Parser::new(tokens, HashSet::new())
        .parse()
        .expect("parser should succeed")
}

#[test]
fn frontend_and_semantic_accepts_valid_program() {
    let source = r#"
        module smoke;

        import std.io;

        fn add(lhs: int, rhs: int) -> int {
            return lhs + rhs;
        }

        pub fn main() -> int {
            let total = add(20, 22);
            println(total);
            return total;
        }
    "#;

    let mut module = parse_module(source);
    let mut modules = vec![&mut module];

    let result = analyze_modules(modules.as_mut_slice());
    assert!(
        result.is_ok(),
        "semantic analysis should succeed: {result:?}"
    );
}

#[test]
fn pipeline_reports_coded_semantic_error() {
    let source = r#"
        module smoke;

        pub fn main() -> int {
            return missing_symbol;
        }
    "#;

    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    let errors = pipeline
        .compile(source, "missing_symbol.spectra")
        .expect_err("compilation should fail");

    assert!(errors.iter().any(|error| matches!(
        error,
        CompilerError::Semantic(semantic) if semantic.code.as_deref() == Some("E001")
    )));
}
