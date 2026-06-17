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

#[test]
fn trait_bound_violation_is_semantic_not_midend() {
    let source = r#"
        module smoke;

        trait Score {
            fn score(&self) -> int;
        }

        struct Plain {
            value: int,
        }

        fn evaluate<T: Score>(item: T) -> int {
            return item.score();
        }

        pub fn main() -> int {
            let plain = Plain { value: 1 };
            return evaluate(plain);
        }
    "#;

    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    let errors = pipeline
        .compile(source, "trait_bound.spectra")
        .expect_err("compilation should fail before midend");

    assert!(errors
        .iter()
        .all(|error| !matches!(error, CompilerError::Midend(_))));
    assert_eq!(
        errors.len(),
        1,
        "expected no cascading diagnostics: {errors:?}"
    );
    assert!(matches!(
        &errors[0],
        CompilerError::Semantic(semantic)
            if semantic.code.as_deref() == Some("E010")
                && semantic.message.contains("Plain")
                && semantic.message.contains("T: Score")
    ));
}

#[test]
fn trait_bound_satisfaction_is_not_item_order_dependent() {
    let source = r#"
        module smoke;

        trait Score {
            fn score(&self) -> int;
        }

        struct Ranked {
            value: int,
        }

        fn evaluate<T: Score>(item: T) -> int {
            return item.score();
        }

        pub fn main() -> int {
            let ranked = Ranked { value: 7 };
            return evaluate(ranked);
        }

        impl Score for Ranked {
            fn score(&self) -> int {
                return self.value;
            }
        }
    "#;

    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    pipeline
        .compile(source, "trait_bound_order.spectra")
        .expect("trait impl declared later should satisfy the generic bound");
}

#[test]
fn unknown_import_alias_member_reports_candidates() {
    let source = r#"
        module smoke;

        import std.math as math;

        pub fn main() -> int {
            return math.not_a_function(1);
        }
    "#;

    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    let errors = pipeline
        .compile(source, "unknown_alias_member.spectra")
        .expect_err("compilation should fail semantically");

    assert!(errors
        .iter()
        .all(|error| !matches!(error, CompilerError::Midend(_))));
    assert_eq!(
        errors.len(),
        1,
        "expected no cascading diagnostics: {errors:?}"
    );
    assert!(matches!(
        &errors[0],
        CompilerError::Semantic(semantic)
            if semantic.code.as_deref() == Some("E011")
                && semantic.message.contains("math")
                && semantic.message.contains("not_a_function")
                && semantic.hint.as_deref().unwrap_or("").contains("sqrt_f")
    ));
}

#[test]
fn std_api_surface_resolves_qualified_and_aliased_calls() {
    let source = r#"
        module api_surface;

        import std.api.http as http;
        import std.api.json as json;
        import std.api.tls as tls;

        pub fn main() -> int {
            let request = http.request_new(1);
            let method = http.request_method(request);
            let method_name = std.api.http.method_name(method);
            let ok = json.validate("{\"ok\": true}");
            let tls_config = tls.client_config();
            let tls_mode = tls.config_mode(tls_config);
            let status_class = std.api.http.status_class(200);
            return status_class + tls_mode;
        }
    "#;

    let mut module = parse_module(source);
    let mut modules = vec![&mut module];

    let result = analyze_modules(modules.as_mut_slice());
    assert!(
        result.is_ok(),
        "std.api semantic surface should resolve without missing-module diagnostics: {result:?}"
    );
}

#[test]
fn generic_return_type_parameter_matches_declared_type_parameter() {
    let source = r#"
        module smoke;

        fn identity<T>(value: T) -> T {
            return value;
        }

        pub fn main() -> int {
            return identity(42);
        }
    "#;

    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    pipeline
        .compile(source, "generic_identity.spectra")
        .expect("generic function returning its declared type parameter should compile");
}

#[test]
fn generic_return_type_parameter_cannot_satisfy_concrete_return() {
    let source = r#"
        module smoke;

        fn bad<T>(value: T) -> string {
            return value;
        }

        pub fn main() -> int {
            let x = bad(1);
            return 0;
        }
    "#;

    let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
    let errors = pipeline
        .compile(source, "generic_bad_return.spectra")
        .expect_err("generic return mismatch should fail in semantic analysis");

    assert!(errors
        .iter()
        .all(|error| !matches!(error, CompilerError::Backend(_) | CompilerError::Midend(_))));
    assert_eq!(
        errors.len(),
        1,
        "expected no backend or cascading diagnostics: {errors:?}"
    );
    assert!(matches!(
        &errors[0],
        CompilerError::Semantic(semantic)
            if semantic.code.as_deref() == Some("E004")
                && semantic.message.contains("expected string")
                && semantic.message.contains("found T")
    ));
}
