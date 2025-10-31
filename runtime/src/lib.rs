//! SpectraLang runtime scaffolding.

use std::io::{self, Write};

/// Initializes the runtime environment.
pub fn initialize() {
    // Placeholder for future GC/runtime bootstrap.
}

/// Console helpers for SpectraLang programs.
pub mod console {
    use super::*;

    /// Writes text to stdout without a trailing newline.
    pub fn print(message: &str) -> io::Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(message.as_bytes())?;
        stdout.flush()
    }

    /// Writes text to stdout with a trailing newline.
    pub fn println(message: &str) -> io::Result<()> {
        print(message)?;
        let mut stdout = io::stdout();
        stdout.write_all(b"\n")?;
        stdout.flush()
    }

    /// Writes text to stderr without a trailing newline.
    pub fn print_err(message: &str) -> io::Result<()> {
        let mut stderr = io::stderr();
        stderr.write_all(message.as_bytes())?;
        stderr.flush()
    }

    /// Writes text to stderr with a trailing newline.
    pub fn println_err(message: &str) -> io::Result<()> {
        print_err(message)?;
        let mut stderr = io::stderr();
        stderr.write_all(b"\n")?;
        stderr.flush()
    }
}

/// Argument helpers for SpectraLang programs.
pub mod args {
    /// Returns the process arguments as owned strings.
    pub fn all() -> Vec<String> {
        std::env::args().collect()
    }

    /// Returns the number of process arguments (including the executable path).
    pub fn len() -> usize {
        std::env::args().len()
    }

    /// Returns true when only the executable path is present.
    pub fn is_empty() -> bool {
        len() <= 1
    }
}
