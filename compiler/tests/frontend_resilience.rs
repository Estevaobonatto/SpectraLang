use spectra_compiler::{CompilationOptions, CompilationPipeline, CompilerError, Lexer, Parser};
use std::collections::HashSet;

#[test]
fn malformed_frontend_inputs_do_not_panic() {
    let corpus = [
        "module demo; fn main( {",
        "module demo; fn main() { let x = \"unterminated; }",
        "module demo; import { println from std.io;",
        "module demo; fn main() { if let = 1 { } }",
        "module demo; fn main() { switch 1 { case => {} } }",
        "module demo; fn main() { loop { ",
    ];

    for source in corpus {
        let _ = Lexer::new(source).tokenize().and_then(|tokens| {
            Parser::new(tokens, HashSet::new())
                .parse()
                .map_err(|_| Vec::new())
        });
    }
}

#[test]
fn promoted_control_flow_constructs_parse_without_feature_flags() {
    let source = r#"
        module stable_control_flow;

        fn classify(value: int) -> int {
            unless value < 0 {
                switch value {
                    case 0 => {
                        return 10;
                    }
                    else => {
                        return 20;
                    }
                }
            } else {
                return -1;
            }
        }

        fn main() -> int {
            let i = 0;
            do {
                i = i + 1;
            } while i < 2;
            loop {
                break;
            }
            return classify(i);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("lexer should succeed");
    Parser::new(tokens, HashSet::new())
        .parse()
        .expect("promoted control-flow constructs should be stable syntax");
}

#[test]
fn lexical_errors_expose_stable_codes() {
    let source = "module bad; fn main() { let x = §§§; }";
    let errors = Lexer::new(source)
        .tokenize()
        .expect_err("lexer should fail on unexpected characters");

    assert!(errors
        .iter()
        .any(|error| error.code.as_deref() == Some("L001")));
}

#[test]
fn pipeline_handles_malformed_inputs_without_internal_errors() {
    let corpus = [
        (
            "broken_import.spectra",
            "module demo; import { println from std.io;",
        ),
        (
            "broken_return.spectra",
            "module demo; fn main() -> int { return; }",
        ),
        (
            "broken_match.spectra",
            "module demo; fn main() { match 1 { case => {} } }",
        ),
    ];

    for (filename, source) in corpus {
        let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
        let result = pipeline.compile(source, filename);
        if let Err(errors) = result {
            assert!(
                errors.iter().all(|error| {
                    !matches!(error, CompilerError::Midend(_) | CompilerError::Backend(_))
                }),
                "frontend-only malformed input should not reach midend/backend: {errors:?}"
            );
        }
    }
}
