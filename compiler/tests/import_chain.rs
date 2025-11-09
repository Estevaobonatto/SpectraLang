use std::fs::{create_dir_all, write};
use std::path::PathBuf;

use tempfile::tempdir;

use spectra_compiler::resolver::{ModuleResolver, ModuleResolverOptions};
use spectra_compiler::semantic::SemanticWorkspace;

#[test]
fn resolves_chained_imports_across_modules() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let root = temp_dir.path();

    write_source(
        root.join("lib/math.spectra"),
        r#"module lib.math;

pub fn double(value: int) -> int {
    return value + value;
}
"#,
    );

    write_source(
        root.join("support/tools.spectra"),
        r#"module support.tools;

import lib.math;

pub fn quad(value: int) -> int {
    return math.double(math.double(value));
}
"#,
    );

    write_source(
        root.join("app.spectra"),
        r#"module app;

import support.tools;

pub fn compute(value: int) -> int {
    return tools.quad(value);
}
"#,
    );

    let entry_path = root.join("app.spectra");

    let mut resolver = ModuleResolver::new(ModuleResolverOptions::default());
    let mut graph = resolver
        .resolve(&entry_path)
        .expect("resolution should succeed for chained imports");

    assert_eq!(
        graph.modules().len(),
        3,
        "expected app, support.tools, lib.math"
    );

    let tools_module = graph
        .modules()
        .iter()
        .find(|module| module.name == "support.tools")
        .expect("support.tools should be part of the graph");
    let math_import = tools_module
        .imports
        .iter()
        .find(|import| import.module == "lib.math" && !import.synthetic)
        .expect("support.tools should import lib.math");
    assert!(
        math_import
            .exposed
            .iter()
            .any(|binding| binding.name == "double"),
        "lib.math should expose the double function to support.tools"
    );

    SemanticWorkspace::analyze(&mut graph)
        .expect("semantic analysis should succeed for chained imports");

    let entry = graph.entry();
    let has_compute = entry
        .ast
        .items
        .iter()
        .any(|item| matches!(item, spectra_compiler::ast::Item::Function(func) if func.name == "compute"));
    assert!(has_compute, "app module should define compute function");

    temp_dir.close().expect("temp dir cleanup should succeed");
}

fn write_source(path: PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        create_dir_all(parent).expect("failed to create parent directories");
    }

    write(&path, contents.as_bytes()).expect("failed to write source file");
}
