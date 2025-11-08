use crate::{CliError, CliResult};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct FormatOptions {
    pub entries: Vec<PathBuf>,
    pub check: bool,
    pub use_stdin: bool,
    pub write_stdout: bool,
}

pub(crate) fn run(options: FormatOptions) -> CliResult<()> {
    if options.use_stdin {
        return format_from_stdin(options.check);
    }

    if options.entries.is_empty() {
        return Err(CliError::usage(
            "No source files or directories were provided for formatting.",
        ));
    }

    let sources = discover_sources(&options.entries)?;
    if sources.is_empty() {
        return Err(CliError::usage(
            "No Spectra source files found in the provided paths.",
        ));
    }

    if options.write_stdout && sources.len() != 1 {
        return Err(CliError::usage(
            "--stdout requires exactly one input file when formatting from disk.",
        ));
    }

    let mut changed = Vec::new();
    let mut processed = 0usize;

    if options.write_stdout {
        let path = &sources[0];
        let original = fs::read_to_string(path).map_err(|error| {
            CliError::io(format!(
                "Failed to read source '{}' for formatting: {}",
                path.display(),
                error
            ))
        })?;

        let output = format_preserving_endings(&original);

        if options.check {
            if output != original {
                return Err(CliError::compilation(format!(
                    "File '{}' is not properly formatted.\nRun 'spectra fmt {}' to produce the formatted output.",
                    path.display(),
                    path.display()
                )));
            }
            return Ok(());
        }

        print!("{}", output);
        return Ok(());
    }

    for path in &sources {
        processed += 1;
        let original = fs::read_to_string(path).map_err(|error| {
            CliError::io(format!(
                "Failed to read source '{}' for formatting: {}",
                path.display(),
                error
            ))
        })?;

        let output = format_preserving_endings(&original);

        if output != original {
            changed.push(path.clone());
            if !options.check {
                write_formatted(path, &output)?;
            }
        }
    }

    if options.check && !changed.is_empty() {
        let mut message =
            String::from("Formatting check failed. The following files need formatting:\n");
        for path in &changed {
            message.push_str("  - ");
            message.push_str(&path.display().to_string());
            message.push('\n');
        }
        message.push_str("Run 'spectra fmt' to apply the required changes.");
        return Err(CliError::compilation(message));
    }

    if options.check {
        println!(
            "Checked {} file{} ({} clean).",
            processed,
            if processed == 1 { "" } else { "s" },
            processed - changed.len()
        );
    } else if changed.is_empty() {
        println!(
            "Formatted {} file{} (no changes needed).",
            processed,
            if processed == 1 { "" } else { "s" }
        );
    } else {
        println!(
            "Formatted {} file{} ({} updated, {} already formatted).",
            processed,
            if processed == 1 { "" } else { "s" },
            changed.len(),
            processed - changed.len()
        );
    }

    Ok(())
}

fn format_preserving_endings(original: &str) -> String {
    let normalized_input = if original.contains("\r\n") {
        original.replace("\r\n", "\n")
    } else {
        original.to_string()
    };

    let formatted = format_source(&normalized_input);

    if original.contains("\r\n") {
        formatted.replace('\n', "\r\n")
    } else {
        formatted
    }
}

fn discover_sources(entries: &[PathBuf]) -> CliResult<Vec<PathBuf>> {
    let mut seen = HashSet::new();
    let mut files = Vec::new();

    for entry in entries {
        let canonical = fs::canonicalize(entry).map_err(|error| {
            CliError::io(format!(
                "Failed to resolve path '{}': {}",
                entry.display(),
                error
            ))
        })?;
        let metadata = fs::symlink_metadata(&canonical).map_err(|error| {
            CliError::io(format!(
                "Failed to inspect '{}': {}",
                canonical.display(),
                error
            ))
        })?;

        if metadata.is_file() {
            if is_source_file(&canonical) {
                files.push(canonical);
            } else {
                return Err(CliError::usage(format!(
                    "Path '{}' is not a Spectra source file (expected .spectra or .spc).",
                    canonical.display()
                )));
            }
            continue;
        }

        if metadata.is_dir() {
            visit_path(&canonical, &mut seen, &mut files)?;
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn format_from_stdin(check: bool) -> CliResult<()> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| CliError::io(format!("Failed to read standard input: {}", error)))?;

    let output = format_preserving_endings(&input);

    if check {
        if output != input {
            return Err(CliError::compilation(
                "Standard input is not properly formatted.\nRun 'spectra fmt --stdin' to rewrite the stream.",
            ));
        }
        return Ok(());
    }

    print!("{}", output);
    Ok(())
}

fn visit_path(path: &Path, seen: &mut HashSet<PathBuf>, out: &mut Vec<PathBuf>) -> CliResult<()> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        CliError::io(format!(
            "Failed to resolve path '{}' during traversal: {}",
            path.display(),
            error
        ))
    })?;

    if !seen.insert(canonical.clone()) {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(&canonical).map_err(|error| {
        CliError::io(format!(
            "Failed to inspect '{}': {}",
            canonical.display(),
            error
        ))
    })?;

    if metadata.is_dir() {
        if should_skip_directory(&canonical) {
            return Ok(());
        }

        let read_dir = fs::read_dir(&canonical).map_err(|error| {
            CliError::io(format!(
                "Failed to enumerate directory '{}': {}",
                canonical.display(),
                error
            ))
        })?;

        for entry in read_dir {
            let entry = entry.map_err(|error| {
                CliError::io(format!(
                    "Failed to enumerate directory '{}': {}",
                    canonical.display(),
                    error
                ))
            })?;
            visit_path(&entry.path(), seen, out)?;
        }
        return Ok(());
    }

    if metadata.is_file() {
        if is_source_file(&canonical) {
            out.push(canonical);
        }
        return Ok(());
    }

    Ok(())
}

fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("spectra") || ext.eq_ignore_ascii_case("spc"))
        .unwrap_or(false)
}

fn should_skip_directory(path: &Path) -> bool {
    match path.file_name().and_then(|value| value.to_str()) {
        Some(name) if name.starts_with('.') => true,
        Some(name) if matches!(name, "target" | "build" | "dist" | "out") => true,
        _ => false,
    }
}

fn write_formatted(path: &Path, contents: &str) -> CliResult<()> {
    fs::write(path, contents).map_err(|error| {
        CliError::io(format!(
            "Failed to write formatted output to '{}': {}",
            path.display(),
            error
        ))
    })
}

fn format_source(input: &str) -> String {
    let mut indent_level: usize = 0;
    let mut formatted_lines = Vec::new();

    for line in input.split('\n') {
        let trimmed_trailing =
            line.trim_end_matches(|ch: char| ch == ' ' || ch == '\t' || ch == '\r');
        let trimmed_leading = trimmed_trailing.trim_start();

        if trimmed_leading.is_empty() {
            formatted_lines.push(String::new());
            continue;
        }

        // Dedent immediately if the line begins with one or more closing braces.
        let mut dedent = count_leading_closing_braces(trimmed_leading);
        if dedent > indent_level {
            dedent = indent_level;
        }

        let indent_for_line = indent_level - dedent;
        let mut line_buffer = String::new();
        line_buffer.extend(std::iter::repeat(' ').take(indent_for_line * 4));
        line_buffer.push_str(trimmed_leading);
        formatted_lines.push(line_buffer);

        let (opens, closes) = count_brace_transitions(trimmed_leading, dedent);
        indent_level = indent_for_line + opens;
        if closes > indent_level {
            indent_level = 0;
        } else {
            indent_level -= closes;
        }
    }

    while matches!(formatted_lines.last(), Some(line) if line.is_empty()) {
        formatted_lines.pop();
    }

    if formatted_lines.is_empty() {
        String::new()
    } else {
        let mut output = formatted_lines.join("\n");
        output.push('\n');
        output
    }
}

fn count_leading_closing_braces(line: &str) -> usize {
    line.chars()
        .take_while(|ch| matches!(ch, '}' | ']' | ')'))
        .count()
}

fn count_brace_transitions(line: &str, mut skip_closing: usize) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    let mut in_char = false;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        if in_char {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '\'' => in_char = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
            }
            '\'' => {
                in_char = true;
            }
            '/' => {
                if matches!(chars.peek(), Some('/')) {
                    break;
                }
            }
            '{' => opens += 1,
            '}' => {
                if skip_closing > 0 {
                    // Leading braces have already been used for dedent, so skip them here.
                    skip_closing -= 1;
                } else {
                    closes += 1;
                }
            }
            _ => {}
        }
    }

    (opens, closes)
}

#[cfg(test)]
mod tests {
    use super::format_source;

    #[test]
    fn formats_basic_block_structure() {
        let input =
            "module demo;\n\nfn main(){\nlet value=1;\nif value>0 {\nprintln(value);\n}\n}\n";
        let expected = "module demo;\n\nfn main() {\n    let value=1;\n    if value>0 {\n        println(value);\n    }\n}\n";
        assert_eq!(format_source(input), expected);
    }

    #[test]
    fn preserves_else_alignment() {
        let input = "fn check(){\nif cond {\nprintln(\"yes\");\n}else{\nprintln(\"no\");\n}\n}\n";
        let expected = "fn check() {\n    if cond {\n        println(\"yes\");\n    } else {\n        println(\"no\");\n    }\n}\n";
        assert_eq!(format_source(input), expected);
    }
}
