// Module discovery — finds .spectra source files in project directories.

use std::fs;
use std::path::{Path, PathBuf};

/// Walk `src_dirs` and collect all `.spectra` / `.spc` files.
/// Files inside directories named `target`, `build`, `dist`, `out`, or those
/// starting with a dot are skipped automatically.
pub fn discover_sources(src_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in src_dirs {
        collect_sources(dir, &mut found);
    }
    found.sort(); // deterministic order
    found
}

fn collect_sources(path: &Path, out: &mut Vec<PathBuf>) {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };

    if metadata.is_dir() {
        if should_skip_directory(path) {
            return;
        }
        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            collect_sources(&entry.path(), out);
        }
    } else if metadata.is_file() && is_source_file(path) {
        if let Ok(abs) = fs::canonicalize(path) {
            out.push(abs);
        } else {
            out.push(path.to_path_buf());
        }
    }
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some(ext) if ext.eq_ignore_ascii_case("spectra") || ext.eq_ignore_ascii_case("spc")
    )
}

fn should_skip_directory(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some(name) if name.starts_with('.') || matches!(name, "target" | "build" | "dist" | "out")
    )
}
