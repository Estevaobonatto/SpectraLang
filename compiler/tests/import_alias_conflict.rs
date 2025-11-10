use std::fs::{create_dir_all, write};
use std::path::Path;

use tempfile::tempdir;

use spectra_compiler::resolver::{ModuleResolver, ModuleResolverOptions};
use spectra_compiler::semantic::SemanticWorkspace;

#[test]
fn duplicate_import_aliases_emit_semantic_error() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let root = temp_dir.path();

    write_module(
        root.join("foo/math.spectra"),
        r#"module foo.math;

pub fn add(a: int, b: int) -> int {
    return a + b;
}
"#,
    );

    write_module(
        root.join("bar/math.spectra"),
        r#"module bar.math;

pub fn add(a: int, b: int) -> int {
    return a + b;
}
"#,
    );

    write_module(
        root.join("app.spectra"),
        r#"module app;

import foo.math;
import bar.math;

pub fn sum(a: int, b: int) -> int {
    return math.add(a, b);
}
"#,
    );

    let entry_path = root.join("app.spectra");
    let mut resolver = ModuleResolver::new(ModuleResolverOptions::default());
    let mut graph = resolver
        .resolve(&entry_path)
        .expect("module resolution should succeed");

    let result = SemanticWorkspace::analyze(&mut graph);

    let errors = result.expect_err("semantic analysis should report duplicate alias conflict");
    let conflict = errors
        .iter()
        .find(|error| error.message.contains("import alias `math`"))
        .expect("expected conflict error for alias math");

    assert!(
        conflict
            .context
            .as_deref()
            .unwrap_or_default()
            .contains("alias `math"),
        "conflict context should mention the original import"
    );
    assert!(
        conflict
            .hint
            .as_deref()
            .unwrap_or_default()
            .contains("use `as`"),
        "conflict hint should recommend aliasing"
    );

    temp_dir.close().expect("temp dir close should succeed");
}

fn write_module(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        create_dir_all(parent).expect("failed to create parent directories");
    }
    write(path, contents.as_bytes()).expect("failed to write module");
}
