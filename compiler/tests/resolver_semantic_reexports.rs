use std::fs::{create_dir_all, write};
use std::path::PathBuf;

use tempfile::tempdir;

use spectra_compiler::ast::Item;
use spectra_compiler::resolver::{ExportKind, ModuleResolver, ModuleResolverOptions};
use spectra_compiler::semantic::SemanticWorkspace;

#[test]
fn resolver_and_semantic_handle_module_reexports() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let root = temp_dir.path();

    write_source(root.join("lib/math.spectra"), r#"module lib.math;

pub fn add(a: int, b: int) -> int {
    return a + b;
}
"#);

    write_source(root.join("lib.spectra"), r#"module lib;

pub import lib.math;
"#);

    write_source(root.join("consumer.spectra"), r#"module consumer;

import lib;

pub fn use_add(x: int, y: int) -> int {
    return lib.math.add(x, y);
}
"#);

    let entry_path = root.join("consumer.spectra");

    let mut resolver = ModuleResolver::new(ModuleResolverOptions::default());
    let mut graph = resolver
        .resolve(&entry_path)
        .expect("module resolution should succeed");

    let entry = graph.entry();
    let lib_import = entry
        .imports
        .iter()
        .find(|import| import.alias == "lib")
        .expect("consumer should import lib");

    let alias_binding = lib_import
        .exposed
        .iter()
        .find(|binding| binding.name == "math")
        .expect("lib should reexport math");

    match &alias_binding.kind {
        ExportKind::ModuleAlias { target } => {
            assert_eq!(target, "lib.math");
        }
        other => panic!("expected module alias, got {:?}", other),
    }
    assert_eq!(alias_binding.origin_module, "lib");

    SemanticWorkspace::analyze(&mut graph)
        .expect("semantic analysis should succeed for reexported call");

    let has_use_add = entry
        .ast
        .items
        .iter()
        .any(|item| matches!(item, Item::Function(func) if func.name == "use_add"));
    assert!(has_use_add, "consumer module should define use_add");

    temp_dir.close().expect("temp dir close should succeed");
}

fn write_source(path: PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        create_dir_all(parent).expect("failed to create parent directories");
    }

    write(&path, contents.as_bytes()).expect("failed to write source file");
}
