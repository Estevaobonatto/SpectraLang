use std::fs::{create_dir_all, write};
use std::path::Path;

use tempfile::tempdir;

use spectra_compiler::resolver::{ModuleResolver, ModuleResolverOptions};
use spectra_compiler::semantic::SemanticWorkspace;

#[test]
fn prelude_allows_invoking_stdlib_aliases_without_prefix() {
    let temp_dir = tempdir().expect("failed to create temp dir");
    let root = temp_dir.path();

    let entry_path = root.join("app.spectra");
    write_source(
        &entry_path,
        r#"module app;

pub fn demo(a: int, b: int) -> int {
    let total = math.add(a, b);
    io.print(total);
    let text_value = text.from_int(total);
    log.record(3, text_value, text_value);
    let _buffer = collections.list_new();
    time.sleep(0);
    return total;
}
"#,
    );

    let mut resolver = ModuleResolver::new(ModuleResolverOptions::default());
    let mut graph = resolver
        .resolve(&entry_path)
        .expect("resolver should inject std.prelude automatically");

    let entry = graph.entry();
    assert!(
        entry
            .imports
            .iter()
            .any(|import| import.module == "std.prelude" && import.synthetic),
        "synthetic std.prelude import should be present"
    );

    SemanticWorkspace::analyze(&mut graph)
        .expect("semantic analysis should succeed with prelude aliases");

    temp_dir.close().expect("temp dir close should succeed");
}

fn write_source(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        create_dir_all(parent).expect("failed to create parent directories");
    }

    write(path, contents.as_bytes()).expect("failed to write source file");
}
