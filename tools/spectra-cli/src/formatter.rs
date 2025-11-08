use crate::{CliError, CliResult};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use toml::Value;

const DEFAULT_INDENT_WIDTH: usize = 4;
const DEFAULT_MAX_LINE_LENGTH: usize = 100;
const MAX_SUPPORTED_INDENT: usize = 12;
const MIN_LINE_LENGTH: usize = 40;

#[derive(Debug, Clone)]
pub(crate) struct FormatterConfig {
    indent_width: usize,
    max_line_length: usize,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            indent_width: DEFAULT_INDENT_WIDTH,
            max_line_length: DEFAULT_MAX_LINE_LENGTH,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FormatOptions {
    pub entries: Vec<PathBuf>,
    pub check: bool,
    pub use_stdin: bool,
    pub write_stdout: bool,
    pub config_path: Option<PathBuf>,
}

pub(crate) fn run(options: FormatOptions) -> CliResult<()> {
    let config = load_formatter_config(&options)?;

    if options.use_stdin {
        return format_from_stdin(options.check, &config);
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

    if options.write_stdout {
        let path = &sources[0];
        let original = fs::read_to_string(path).map_err(|error| {
            CliError::io(format!(
                "Failed to read source '{}' for formatting: {}",
                path.display(),
                error
            ))
        })?;

        let output = format_preserving_endings(&original, &config);

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

    let mut changed = Vec::new();
    let mut processed = 0usize;

    for path in &sources {
        processed += 1;
        let original = fs::read_to_string(path).map_err(|error| {
            CliError::io(format!(
                "Failed to read source '{}' for formatting: {}",
                path.display(),
                error
            ))
        })?;

        let output = format_preserving_endings(&original, &config);

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

fn load_formatter_config(options: &FormatOptions) -> CliResult<FormatterConfig> {
    if let Some(path) = &options.config_path {
        let canonical = fs::canonicalize(path).map_err(|error| {
            CliError::io(format!(
                "Failed to resolve configuration path '{}': {}",
                path.display(),
                error
            ))
        })?;

        if !canonical.is_file() {
            return Err(CliError::usage(format!(
                "Configuration override '{}' is not a file.",
                canonical.display()
            )));
        }

        return parse_formatter_config(&canonical);
    }

    let mut search_roots = resolve_search_roots(options)?;
    search_roots.sort();
    search_roots.dedup();

    for root in search_roots {
        if let Some(config_path) = find_config_file(&root) {
            return parse_formatter_config(&config_path);
        }
    }

    Ok(FormatterConfig::default())
}

fn resolve_search_roots(options: &FormatOptions) -> CliResult<Vec<PathBuf>> {
    let mut roots = Vec::new();

    if options.use_stdin {
        let cwd = env::current_dir().map_err(|error| {
            CliError::io(format!("Failed to determine current directory: {}", error))
        })?;
        roots.push(cwd);
    }

    for entry in &options.entries {
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

        let base = if metadata.is_file() {
            canonical
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            canonical
        };

        roots.push(base);
    }

    Ok(roots)
}

fn find_config_file(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);

    while let Some(dir) = current {
        let candidate = dir.join("Spectra.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }

    None
}

fn parse_formatter_config(path: &Path) -> CliResult<FormatterConfig> {
    let manifest = fs::read_to_string(path).map_err(|error| {
        CliError::io(format!(
            "Failed to read configuration '{}': {}",
            path.display(),
            error
        ))
    })?;

    let value: Value = manifest.parse().map_err(|error| {
        CliError::usage(format!("Failed to parse '{}': {}", path.display(), error))
    })?;

    let formatter = match value.get("formatter") {
        None => return Ok(FormatterConfig::default()),
        Some(Value::Table(table)) => table,
        Some(_) => {
            return Err(CliError::usage(format!(
                "Section [formatter] in '{}' must be a table.",
                path.display()
            )))
        }
    };

    let mut config = FormatterConfig::default();

    for (key, value) in formatter {
        match key.as_str() {
            "indent_width" => {
                config.indent_width = parse_positive_usize(
                    value,
                    "indent_width",
                    path,
                    1,
                    MAX_SUPPORTED_INDENT,
                )?;
            }
            "max_line_length" => {
                config.max_line_length = parse_positive_usize(
                    value,
                    "max_line_length",
                    path,
                    MIN_LINE_LENGTH,
                    usize::MAX,
                )?;
            }
            other => {
                return Err(CliError::usage(format!(
                    "Unknown formatter option '{}' in '{}'.",
                    other,
                    path.display()
                )));
            }
        }
    }

    Ok(config)
}

fn parse_positive_usize(
    value: &Value,
    key: &str,
    path: &Path,
    min: usize,
    max: usize,
) -> CliResult<usize> {
    if let Some(raw) = value.as_integer() {
        if raw < min as i64 || raw > max as i64 {
            return Err(CliError::usage(format!(
                "Value '{}' in '{}' must be between {} and {}.",
                key,
                path.display(),
                min,
                max
            )));
        }
        Ok(raw as usize)
    } else {
        Err(CliError::usage(format!(
            "Value '{}' in '{}' must be an integer.",
            key,
            path.display()
        )))
    }
}

fn format_preserving_endings(original: &str, config: &FormatterConfig) -> String {
    let normalized_input = if original.contains("\r\n") {
        original.replace("\r\n", "\n")
    } else {
        original.to_string()
    };

    let formatted = format_source(&normalized_input, config);

    if original.contains("\r\n") {
        formatted.replace('\n', "\r\n")
    } else {
        formatted
    }
}

fn format_from_stdin(check: bool, config: &FormatterConfig) -> CliResult<()> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| CliError::io(format!("Failed to read standard input: {}", error)))?;

    let output = format_preserving_endings(&input, config);

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

    if metadata.is_file() && is_source_file(&canonical) {
        out.push(canonical);
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

#[derive(Debug, Clone)]
struct FormattedLine {
    indent_level: usize,
    content: String,
    is_blank: bool,
}

impl FormattedLine {
    fn blank() -> Self {
        Self {
            indent_level: 0,
            content: String::new(),
            is_blank: true,
        }
    }

    fn new(indent_level: usize, content: String) -> Self {
        Self {
            indent_level,
            content,
            is_blank: false,
        }
    }
}

fn format_source(input: &str, config: &FormatterConfig) -> String {
    let mut indent_level: usize = 0;
    let mut lines = Vec::new();

    for line in input.split('\n') {
        let trimmed_trailing =
            line.trim_end_matches(|ch: char| ch == ' ' || ch == '\t' || ch == '\r');
        let trimmed_leading = trimmed_trailing.trim_start();

        if trimmed_leading.is_empty() {
            lines.push(FormattedLine::blank());
            continue;
        }

        let mut dedent = count_leading_closing_braces(trimmed_leading);
        if dedent > indent_level {
            dedent = indent_level;
        }

        let indent_for_line = indent_level.saturating_sub(dedent);
        let normalized = normalize_spacing(trimmed_leading);
        lines.push(FormattedLine::new(indent_for_line, normalized));

        let (opens, closes) = count_brace_transitions(trimmed_leading, dedent);
        indent_level = indent_for_line + opens;
        indent_level = indent_level.saturating_sub(closes);
    }

    align_let_bindings(&mut lines, config);

    let mut output_lines = Vec::new();
    let mut blank_streak = 0usize;

    for line in lines {
        if line.is_blank {
            blank_streak += 1;
            if blank_streak > 1 {
                continue;
            }
            output_lines.push(String::new());
            continue;
        }

        blank_streak = 0;
        let mut buffer = String::new();
        buffer.extend(std::iter::repeat(' ').take(line.indent_level * config.indent_width));
        buffer.push_str(&line.content);
        output_lines.push(buffer);
    }

    while matches!(output_lines.last(), Some(value) if value.is_empty()) {
        output_lines.pop();
    }

    if output_lines.is_empty() {
        String::new()
    } else {
        output_lines.join("\n") + "\n"
    }
}

fn normalize_spacing(content: &str) -> String {
    let mut result = String::new();
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut in_char = false;
    let mut escape = false;
    let mut pending_space = false;

    while let Some(ch) = chars.next() {
        if in_string {
            result.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if in_char {
            result.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_char = false;
            }
            continue;
        }

        match ch {
            '"' => {
                push_pending_space(&mut result, &mut pending_space);
                result.push('"');
                in_string = true;
            }
            '\'' => {
                push_pending_space(&mut result, &mut pending_space);
                result.push('\'');
                in_char = true;
            }
            ch if ch.is_whitespace() => {
                pending_space = true;
            }
            ':' => {
                while result.ends_with(' ') {
                    result.pop();
                }
                if matches!(chars.peek(), Some(':')) {
                    result.push(':');
                    result.push(':');
                    chars.next();
                    pending_space = false;
                } else {
                    result.push(':');
                    pending_space = true;
                }
            }
            ',' | ';' => {
                while result.ends_with(' ') {
                    result.pop();
                }
                result.push(ch);
                pending_space = true;
            }
            ')' | ']' | '}' => {
                while result.ends_with(' ') {
                    result.pop();
                }
                result.push(ch);
                pending_space = true;
            }
            '(' | '[' | '{' => {
                push_pending_space(&mut result, &mut pending_space);
                result.push(ch);
            }
            '/' => {
                if matches!(chars.peek(), Some('/')) {
                    if !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.push('/');
                    result.push('/');
                    chars.next();
                    while let Some(next) = chars.next() {
                        result.push(next);
                    }
                    break;
                }
                push_pending_space(&mut result, &mut pending_space);
                result.push('/');
            }
            _ => {
                if let Some(op) = read_operator(ch, &mut chars) {
                    let prev = previous_non_space(&result);
                    let is_unary_minus =
                        op == "-" && matches!(prev, None | Some('(' | '[' | '{' | '=' | ',' | ':'));
                    let is_unary_not =
                        op == "!" && matches!(prev, None | Some('(' | '[' | '{' | '=' | ',' | ':'));

                    if is_unary_minus || is_unary_not {
                        push_pending_space(&mut result, &mut pending_space);
                        result.push_str(&op);
                    } else {
                        if !result.is_empty() && !result.ends_with(' ') {
                            result.push(' ');
                        }
                        result.push_str(&op);
                        pending_space = true;
                    }
                    continue;
                }

                push_pending_space(&mut result, &mut pending_space);
                result.push(ch);
            }
        }
    }

    result.trim_end().to_string()
}

fn read_operator(
    first: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Option<String> {
    let mut op = String::new();
    op.push(first);

    match first {
        '=' => match chars.peek() {
            Some('=') => {
                chars.next();
                op.push('=');
                Some(op)
            }
            Some('>') => {
                chars.next();
                op.push('>');
                Some(op)
            }
            _ => Some(op),
        },
        '!' => match chars.peek() {
            Some('=') => {
                chars.next();
                op.push('=');
                Some(op)
            }
            _ => Some(op),
        },
        '<' => match chars.peek() {
            Some('=') => {
                chars.next();
                op.push('=');
                Some(op)
            }
            _ => Some(op),
        },
        '>' => match chars.peek() {
            Some('=') => {
                chars.next();
                op.push('=');
                Some(op)
            }
            _ => Some(op),
        },
        '&' => match chars.peek() {
            Some('&') => {
                chars.next();
                op.push('&');
                Some(op)
            }
            _ => Some(op),
        },
        '|' => match chars.peek() {
            Some('|') => {
                chars.next();
                op.push('|');
                Some(op)
            }
            _ => Some(op),
        },
        '+' | '*' | '/' | '%' => Some(op),
        '-' => match chars.peek() {
            Some('>') => {
                chars.next();
                op.push('>');
                Some(op)
            }
            Some('=') => {
                chars.next();
                op.push('=');
                Some(op)
            }
            _ => Some(op),
        },
        _ => None,
    }
}

fn previous_non_space(result: &str) -> Option<char> {
    result.chars().rev().find(|ch| !ch.is_whitespace())
}

fn push_pending_space(result: &mut String, pending: &mut bool) {
    if *pending && !result.is_empty() && !result.ends_with(' ') {
        result.push(' ');
    }
    *pending = false;
}

fn align_let_bindings(lines: &mut [FormattedLine], config: &FormatterConfig) {
    let mut index = 0usize;

    while index < lines.len() {
        if lines[index].is_blank {
            index += 1;
            continue;
        }

        let indent = lines[index].indent_level;
        if !lines[index].content.starts_with("let ") {
            index += 1;
            continue;
        }

        let mut group = Vec::new();
        let mut cursor = index;

        while cursor < lines.len() {
            if lines[cursor].is_blank || lines[cursor].indent_level != indent {
                break;
            }
            if !lines[cursor].content.starts_with("let ") {
                break;
            }
            if let Some(split) = lines[cursor].content.split_once(" = ") {
                group.push((cursor, split.0.to_string(), split.1.to_string()));
                cursor += 1;
            } else {
                break;
            }
        }

        if group.len() < 2 {
            index += 1;
            continue;
        }

        let max_binding = group
            .iter()
            .map(|(_, before, _)| before.len())
            .max()
            .unwrap_or(0);
        let target_column = max_binding + 1;

        let exceeds_limit = group.iter().any(|(_, _, after)| {
            let line_length = indent * config.indent_width + target_column + 2 + after.len();
            line_length > config.max_line_length
        });

        if exceeds_limit {
            index = cursor;
            continue;
        }

        for (line_index, before, after) in group {
            let padding = target_column.saturating_sub(before.len());
            let mut rebuilt = before;
            rebuilt.extend(std::iter::repeat(' ').take(padding));
            rebuilt.push_str("= ");
            rebuilt.push_str(&after);
            lines[line_index].content = rebuilt;
        }

        index = cursor;
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
            '"' => in_string = true,
            '\'' => in_char = true,
            '/' => {
                if matches!(chars.peek(), Some('/')) {
                    break;
                }
            }
            '{' | '(' | '[' => opens += 1,
            '}' | ')' | ']' => {
                if skip_closing > 0 {
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
    use super::{format_source, FormatterConfig};

    #[test]
    fn formats_basic_block_structure() {
        let input =
            "module demo;\n\nfn main(){\nlet value=1;\nif value>0 {\nprintln(value);\n}\n}\n";
        let expected =
            "module demo;\n\nfn main() {\n    let value = 1;\n    if value > 0 {\n        println(value);\n    }\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn preserves_else_alignment() {
        let input = "fn check(){\nif cond {\nprintf(\"yes\");\n}else{\nprintf(\"no\");\n}\n}\n";
        let expected =
            "fn check() {\n    if cond {\n        printf(\"yes\");\n    } else {\n        printf(\"no\");\n    }\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn inserts_spaces_around_binary_operators() {
        let input = "fn math(){\nlet sum=left+right*factor-3;\nlet cmp=a==b||a!=c&&d>=e;\n}\n";
        let expected =
            "fn math() {\n    let sum = left + right * factor - 3;\n    let cmp = a == b || a != c && d >= e;\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn aligns_consecutive_let_bindings() {
        let input =
            "fn demo(){\nlet short=1;\nlet much_longer_name=2;\nlet mid=short+much_longer_name;\n}\n";
        let expected =
            "fn demo() {\n    let short             = 1;\n    let much_longer_name = 2;\n    let mid              = short + much_longer_name;\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn respects_custom_indent_width() {
        let mut config = FormatterConfig::default();
        config.indent_width = 2;
        let input = "fn main(){\nif cond {\nprintln(\"hi\");\n}\n}\n";
        let expected = "fn main() {\n  if cond {\n    println(\"hi\");\n  }\n}\n";
        assert_eq!(format_source(input, &config), expected);
    }

    #[test]
    fn skips_alignment_when_line_would_exceed_limit() {
        let mut config = FormatterConfig::default();
        config.max_line_length = 40;
        let input =
            "fn wide(){\nlet short=call();\nlet very_very_long_identifier=call_with_many_arguments();\n}\n";
        let expected =
            "fn wide() {\n    let short = call();\n    let very_very_long_identifier = call_with_many_arguments();\n}\n";
        assert_eq!(format_source(input, &config), expected);
    }
}
