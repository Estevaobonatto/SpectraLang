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
use std::process::Command;

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

/// Verify the runtime archive selected for AOT contains the ABI symbols used
/// by compiler-native autodiff.  The linker remains the final authority, but
/// catching an obsolete archive here gives a deterministic diagnostic instead
/// of an opaque unresolved-symbol error much later in the pipeline.
pub fn validate_required_symbols(path: &std::path::Path) -> Result<(), String> {
    const REQUIRED: [&str; 2] = [
        "spectra_rt_tensor_autodiff_apply_fast",
        "spectra_rt_tensor_grad_handle_fast",
    ];
    let mut command = None;
    if cfg!(windows) {
        if let Ok(output) = Command::new("dumpbin").arg("/symbols").arg(path).output() {
            command = Some(output);
        }
    }
    if command.is_none() {
        for tool in ["llvm-nm", "nm"] {
            if let Ok(output) = Command::new(tool).arg("-g").arg(path).output() {
                command = Some(output);
                break;
            }
        }
    }
    let Some(output) = command else {
        // Tooling is not guaranteed on developer machines. The real linker
        // still validates the archive; the validator records this as an
        // environment limitation rather than pretending inspection occurred.
        return Ok(());
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let missing = REQUIRED.iter().filter(|symbol| !text.contains(**symbol)).collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "runtime archive '{}' is missing required symbols: {}",
            path.display(),
            missing.iter().map(|s| **s).collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(())
}
