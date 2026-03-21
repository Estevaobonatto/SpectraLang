/// Integration tests for the complete compiler pipeline
use spectra_compiler::{CompilationOptions, CompilationPipeline};
use std::fs;

fn compile_source(source: &str) -> Result<(), String> {
    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    pipeline
        .compile(source, "integration_test.spectra")
        .map(|_| ())
        .map_err(|errors| {
            errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        })
}

#[test]
fn test_compile_simple_test() {
    let source = fs::read_to_string("../../examples/simple_test.spectra")
        .expect("Failed to read simple_test.spectra");

    assert!(!source.is_empty(), "Source file should not be empty");
    let result = compile_source(&source);
    assert!(result.is_ok(), "simple_test.spectra should compile: {:?}", result);
}

#[test]
fn test_compile_math_functions() {
    let source = fs::read_to_string("../../examples/math_functions.spectra")
        .expect("Failed to read math_functions.spectra");

    assert!(!source.is_empty(), "Source file should not be empty");
    let result = compile_source(&source);
    assert!(result.is_ok(), "math_functions.spectra should compile: {:?}", result);
}

#[test]
fn test_compile_test_optimization() {
    let source = fs::read_to_string("../../examples/test_optimization.spectra")
        .expect("Failed to read test_optimization.spectra");

    assert!(!source.is_empty(), "Source file should not be empty");
    let result = compile_source(&source);
    assert!(result.is_ok(), "test_optimization.spectra should compile: {:?}", result);
}

#[test]
fn test_inline_basic_program() {
    let source = r#"
        module integration_test;

        fn add(a: int, b: int) -> int {
            return a + b;
        }

        fn main() -> int {
            let result = add(3, 4);
            return result;
        }
    "#;
    let result = compile_source(source);
    assert!(result.is_ok(), "inline basic program should compile: {:?}", result);
}

#[test]
fn test_inline_if_else() {
    let source = r#"
        module integration_test;

        fn max(a: int, b: int) -> int {
            if a > b {
                return a;
            } else {
                return b;
            }
        }
    "#;
    let result = compile_source(source);
    assert!(result.is_ok(), "if-else program should compile: {:?}", result);
}

#[test]
fn test_inline_while_loop() {
    let source = r#"
        module integration_test;

        fn sum_to(n: int) -> int {
            let total = 0;
            let i = 0;
            while i <= n {
                total = total + i;
                i = i + 1;
            }
            return total;
        }
    "#;
    let result = compile_source(source);
    assert!(result.is_ok(), "while loop program should compile: {:?}", result);
}
