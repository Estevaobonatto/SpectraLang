use spectra_compiler::{Lexer, Parser};
use spectra_midend::ir::pretty::format_module;
use spectra_midend::ASTLowering;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

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

#[test]
fn ir_snapshot_covers_lowering_stage() {
    let source = r#"
        module lowering_snapshot;

        fn add(lhs: int, rhs: int) -> int {
            let total = lhs + rhs;
            return total;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("lexing should pass");
    let ast = Parser::new(tokens, HashSet::new())
        .parse()
        .expect("parsing should pass");
    let ir = ASTLowering::new()
        .lower_module(&ast)
        .expect("lowering should pass");

    assert_snapshot("lowering_ir.snap", &format_module(&ir));
}
