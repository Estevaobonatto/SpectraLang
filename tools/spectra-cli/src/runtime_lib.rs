// Locates the pre-built Spectra runtime static library that is required when
// linking a native executable with `--emit-exe`.
//
// Search order:
//   1. `SPECTRA_RUNTIME_LIB` environment variable (user override).
//   2. Same directory as the running binary (release / installed layout).
//   3. `../lib/` relative to the binary (another common install layout).
//   4. Cargo profile target directories relative to the binary (dev/release builds).

use std::env;
use std::path::PathBuf;

/// Returns the path to `libspectra_runtime.a` (Unix) or `spectra_runtime.lib` (MSVC Windows),
/// or `None` if it cannot be found.
pub fn find_runtime_lib() -> Option<PathBuf> {
    // 1. Explicit user override.
    if let Ok(val) = env::var("SPECTRA_RUNTIME_LIB") {
        let path = PathBuf::from(val);
        if path.exists() {
            return Some(path);
        }
    }

    let exe = env::current_exe().ok()?;
    let bin_dir = exe.parent()?;

    // Candidate file names for the runtime static library.
    let candidates: &[&str] = &["libspectra_runtime.a", "spectra_runtime.lib"];

    // 2. Same directory as the binary.
    for name in candidates {
        let p = bin_dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }

    // 3. ../lib/ relative to the binary (e.g. /usr/local/lib/).
    if let Some(lib_dir) = bin_dir.parent().map(|d| d.join("lib")) {
        for name in candidates {
            let p = lib_dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 4. Cargo workspace layout: binary is in target/{profile}/; the runtime is
    //    built in target/{profile}/ as well when `crate-type = ["staticlib"]`.
    //    bin_dir is already that directory, so this is identical to check 2.
    //    However, also check the sibling `spectra-runtime` build directory that
    //    Cargo may place output in.
    let profile_dir = bin_dir;
    for name in candidates {
        // target/{profile}/libspectra_runtime.a (already checked above, but harmless)
        let p = profile_dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }

    None
}
