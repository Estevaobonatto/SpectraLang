/// Integration tests for the complete compiler pipeline
use std::fs;

#[test]
fn test_compile_simple_test() {
    let source = fs::read_to_string("../../examples/simple_test.spectra")
        .expect("Failed to read simple_test.spectra");

    // This would normally compile the source
    // For now, we just verify the file exists and can be read
    assert!(!source.is_empty(), "Source file should not be empty");
    assert!(
        source.contains("module test"),
        "Should contain module declaration"
    );
}

#[test]
fn test_compile_math_functions() {
    let source = fs::read_to_string("../../examples/math_functions.spectra")
        .expect("Failed to read math_functions.spectra");

    assert!(!source.is_empty(), "Source file should not be empty");
    assert!(
        source.contains("factorial"),
        "Should contain factorial function"
    );
    assert!(
        source.contains("fibonacci"),
        "Should contain fibonacci function"
    );
}

#[test]
fn test_compile_test_optimization() {
    let source = fs::read_to_string("../../examples/test_optimization.spectra")
        .expect("Failed to read test_optimization.spectra");

    assert!(!source.is_empty(), "Source file should not be empty");
    assert!(source.contains("if c > 20"), "Should contain conditional");
}

// TODO: Implement actual compilation tests once the compiler API is stable
// These placeholder tests verify that test files exist and can be read
