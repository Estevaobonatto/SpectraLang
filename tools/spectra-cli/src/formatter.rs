use crate::{CliError, CliResult};
use diff::{lines, Result as DiffResult};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::Write as FmtWrite;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExplainMode {
    None,
    Text,
    Json,
}

#[derive(Debug)]
pub(crate) struct FormatOptions {
    pub entries: Vec<PathBuf>,
    pub check: bool,
    pub use_stdin: bool,
    pub write_stdout: bool,
    pub explain: ExplainMode,
    pub stats: bool,
    pub config_path: Option<PathBuf>,
}

struct FormatterConfigResolver {
    override_config: Option<FormatterConfig>,
    directory_cache: HashMap<PathBuf, FormatterConfig>,
    manifest_cache: HashMap<PathBuf, Option<PathBuf>>,
    parsed_configs: HashMap<PathBuf, FormatterConfig>,
    default: FormatterConfig,
    stats: ConfigStats,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ConfigStats {
    cache_lookups: usize,
    cache_hits: usize,
    cache_misses: usize,
}

impl ConfigStats {
    fn record_hit(&mut self) {
        self.cache_lookups += 1;
        self.cache_hits += 1;
    }

    fn record_miss(&mut self) {
        self.cache_lookups += 1;
        self.cache_misses += 1;
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
struct FormatterRunStats {
    processed: usize,
    changed: usize,
    updated: usize,
    unchanged: usize,
    mode: FormatterMode,
    config_cache_lookups: usize,
    config_cache_hits: usize,
    config_cache_misses: usize,
}

impl FormatterRunStats {
    fn new(processed: usize, changed: usize, is_check: bool, config_stats: &ConfigStats) -> Self {
        let unchanged = processed.saturating_sub(changed);
        let updated = if is_check { 0 } else { changed };
        Self {
            processed,
            changed,
            updated,
            unchanged,
            mode: if is_check {
                FormatterMode::Check
            } else {
                FormatterMode::Write
            },
            config_cache_lookups: config_stats.cache_lookups,
            config_cache_hits: config_stats.cache_hits,
            config_cache_misses: config_stats.cache_misses,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum FormatterMode {
    Check,
    Write,
}

impl FormatterConfigResolver {
    fn new(options: &FormatOptions) -> CliResult<Self> {
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

            let config = parse_formatter_config(&canonical)?;

            return Ok(Self {
                override_config: Some(config),
                directory_cache: HashMap::new(),
                manifest_cache: HashMap::new(),
                parsed_configs: HashMap::new(),
                default: FormatterConfig::default(),
                stats: ConfigStats::default(),
            });
        }

        Ok(Self {
            override_config: None,
            directory_cache: HashMap::new(),
            manifest_cache: HashMap::new(),
            parsed_configs: HashMap::new(),
            default: FormatterConfig::default(),
            stats: ConfigStats::default(),
        })
    }

    fn config_for_stdin(&mut self) -> CliResult<FormatterConfig> {
        if let Some(config) = &self.override_config {
            return Ok(config.clone());
        }

        let cwd = env::current_dir().map_err(|error| {
            CliError::io(format!("Failed to determine current directory: {}", error))
        })?;
        let canonical = fs::canonicalize(&cwd).map_err(|error| {
            CliError::io(format!(
                "Failed to resolve current directory '{}': {}",
                cwd.display(),
                error
            ))
        })?;

        self.config_for_directory(&canonical)
    }

    fn config_for_path(&mut self, path: &Path) -> CliResult<FormatterConfig> {
        if let Some(config) = &self.override_config {
            return Ok(config.clone());
        }

        let canonical = fs::canonicalize(path).map_err(|error| {
            CliError::io(format!(
                "Failed to resolve path '{}': {}",
                path.display(),
                error
            ))
        })?;

        let directory = canonical
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        self.config_for_directory(&directory)
    }

    fn config_for_directory(&mut self, directory: &Path) -> CliResult<FormatterConfig> {
        if let Some(config) = self.directory_cache.get(directory) {
            self.stats.record_hit();
            return Ok(config.clone());
        }

        self.stats.record_miss();

        let manifest = self.manifest_for_directory(directory)?;

        let config = if let Some(manifest_path) = manifest {
            if let Some(existing) = self.parsed_configs.get(&manifest_path) {
                existing.clone()
            } else {
                let parsed = parse_formatter_config(&manifest_path)?;
                self.parsed_configs
                    .insert(manifest_path.clone(), parsed.clone());
                parsed
            }
        } else {
            self.default.clone()
        };

        self.directory_cache
            .insert(directory.to_path_buf(), config.clone());

        Ok(config)
    }

    fn stats(&self) -> ConfigStats {
        self.stats.clone()
    }

    fn manifest_for_directory(&mut self, directory: &Path) -> CliResult<Option<PathBuf>> {
        if let Some(cached) = self.manifest_cache.get(directory) {
            return Ok(cached.clone());
        }

        let mut current = Some(directory.to_path_buf());
        let mut visited = Vec::new();
        let mut result = None;

        while let Some(dir) = current {
            if let Some(cached) = self.manifest_cache.get(&dir) {
                result = cached.clone();
                break;
            }

            visited.push(dir.clone());
            let candidate = dir.join("Spectra.toml");
            if candidate.is_file() {
                let canonical = fs::canonicalize(&candidate).map_err(|error| {
                    CliError::io(format!(
                        "Failed to resolve configuration '{}': {}",
                        candidate.display(),
                        error
                    ))
                })?;
                result = Some(canonical);
                break;
            }

            current = dir.parent().map(Path::to_path_buf);
        }

        for dir in visited {
            self.manifest_cache.insert(dir, result.clone());
        }

        Ok(result)
    }
}

pub(crate) fn run(options: FormatOptions) -> CliResult<()> {
    if options.explain != ExplainMode::None && options.use_stdin {
        return Err(CliError::usage(
            "--explain cannot be used with --stdin. Provide files or directories instead.",
        ));
    }

    if options.explain != ExplainMode::None && options.write_stdout {
        return Err(CliError::usage(
            "--explain cannot be combined with --stdout. Use regular formatting or --check.",
        ));
    }

    let mut config_resolver = FormatterConfigResolver::new(&options)?;

    if options.use_stdin {
        let config = config_resolver.config_for_stdin()?;
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(|error| CliError::io(format!("Failed to read standard input: {}", error)))?;

        let output = format_preserving_endings(&input, &config);
        let changed = output != input;
        let run_stats = {
            let config_stats = config_resolver.stats();
            FormatterRunStats::new(1, usize::from(changed), options.check, &config_stats)
        };

        if options.check {
            if changed {
                maybe_emit_stats(&options, &run_stats);
                return Err(CliError::compilation(
                    "Standard input is not properly formatted.\nRun 'spectra fmt --stdin' to rewrite the stream.",
                ));
            }

            maybe_emit_stats(&options, &run_stats);
            return Ok(());
        }

        print!("{}", output);
        maybe_emit_stats(&options, &run_stats);
        return Ok(());
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
        let config = config_resolver.config_for_path(path)?;
        let original = fs::read_to_string(path).map_err(|error| {
            CliError::io(format!(
                "Failed to read source '{}' for formatting: {}",
                path.display(),
                error
            ))
        })?;

        let output = format_preserving_endings(&original, &config);
        let changed = output != original;
        let run_stats = {
            let config_stats = config_resolver.stats();
            FormatterRunStats::new(1, usize::from(changed), options.check, &config_stats)
        };

        if options.check {
            if changed {
                maybe_emit_stats(&options, &run_stats);
                return Err(CliError::compilation(format!(
                    "File '{}' is not properly formatted.\nRun 'spectra fmt {}' to produce the formatted output.",
                    path.display(),
                    path.display()
                )));
            }
            maybe_emit_stats(&options, &run_stats);
            return Ok(());
        }

        print!("{}", output);
        maybe_emit_stats(&options, &run_stats);
        return Ok(());
    }

    let mut changed = Vec::new();
    let mut text_diffs = if options.explain == ExplainMode::Text {
        Some(Vec::new())
    } else {
        None
    };
    let mut json_diffs = if options.explain == ExplainMode::Json {
        Some(Vec::new())
    } else {
        None
    };
    let mut processed = 0usize;

    for path in &sources {
        processed += 1;
        let config = config_resolver.config_for_path(path)?;
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
            if let Some(diffs) = text_diffs.as_mut() {
                diffs.push(render_diff(path, &original, &output));
            }
            if let Some(diffs) = json_diffs.as_mut() {
                diffs.push(render_json_diff(path, &original, &output));
            }
            if !options.check {
                write_formatted(path, &output)?;
            }
        }
    }

    let run_stats = {
        let config_stats = config_resolver.stats();
        FormatterRunStats::new(processed, changed.len(), options.check, &config_stats)
    };

    match options.explain {
        ExplainMode::Text => {
            let diffs = text_diffs.unwrap_or_default();
            if changed.is_empty() {
                println!(
                    "No formatting changes detected across {} file{}.",
                    processed,
                    if processed == 1 { "" } else { "s" }
                );
                maybe_emit_stats(&options, &run_stats);
                return Ok(());
            }

            for diff in diffs {
                println!("{}", diff);
            }

            maybe_emit_stats(&options, &run_stats);
            return Err(CliError::compilation(format!(
                "Formatting differences detected in {} file{}.",
                changed.len(),
                if changed.len() == 1 { "" } else { "s" }
            )));
        }
        ExplainMode::Json => {
            let diffs = json_diffs.unwrap_or_default();
            let payload = JsonExplainPayload {
                summary: JsonSummary::from_stats(&run_stats),
                files: diffs,
            };

            let serialized = serde_json::to_string_pretty(&payload).map_err(|error| {
                CliError::io(format!("Failed to serialize explain payload: {}", error))
            })?;

            println!("{}", serialized);

            if changed.is_empty() {
                maybe_emit_stats(&options, &run_stats);
                return Ok(());
            }

            maybe_emit_stats(&options, &run_stats);
            return Err(CliError::compilation(format!(
                "Formatting differences detected in {} file{}.",
                changed.len(),
                if changed.len() == 1 { "" } else { "s" }
            )));
        }
        ExplainMode::None => {}
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
        maybe_emit_stats(&options, &run_stats);
        return Err(CliError::compilation(message));
    }

    if options.check {
        println!(
            "Checked {} file{} ({} clean).",
            processed,
            if processed == 1 { "" } else { "s" },
            processed - changed.len()
        );
        maybe_emit_stats(&options, &run_stats);
    } else if changed.is_empty() {
        println!(
            "Formatted {} file{} (no changes needed).",
            processed,
            if processed == 1 { "" } else { "s" }
        );
        maybe_emit_stats(&options, &run_stats);
    } else {
        println!(
            "Formatted {} file{} ({} updated, {} already formatted).",
            processed,
            if processed == 1 { "" } else { "s" },
            changed.len(),
            processed - changed.len()
        );
        maybe_emit_stats(&options, &run_stats);
    }

    Ok(())
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
                config.indent_width =
                    parse_positive_usize(value, "indent_width", path, 1, MAX_SUPPORTED_INDENT)?;
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

fn render_diff(path: &Path, original: &str, formatted: &str) -> String {
    let mut buffer = String::new();
    let _ = writeln!(buffer, "diff --spectra {}", path.display());
    let _ = writeln!(buffer, "--- original");
    let _ = writeln!(buffer, "+++ formatted");

    for change in lines(original, formatted) {
        match change {
            DiffResult::Left(line) => {
                let _ = writeln!(buffer, "-{}", line);
            }
            DiffResult::Right(line) => {
                let _ = writeln!(buffer, "+{}", line);
            }
            DiffResult::Both(line, _) => {
                let _ = writeln!(buffer, " {}", line);
            }
        }
    }

    buffer
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct JsonExplainPayload {
    summary: JsonSummary,
    files: Vec<JsonFileDiff>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct JsonSummary {
    processed: usize,
    changed: usize,
    updated: usize,
    unchanged: usize,
    mode: FormatterMode,
    config_cache_lookups: usize,
    config_cache_hits: usize,
    config_cache_misses: usize,
}

impl JsonSummary {
    fn from_stats(stats: &FormatterRunStats) -> Self {
        Self {
            processed: stats.processed,
            changed: stats.changed,
            updated: stats.updated,
            unchanged: stats.unchanged,
            mode: stats.mode,
            config_cache_lookups: stats.config_cache_lookups,
            config_cache_hits: stats.config_cache_hits,
            config_cache_misses: stats.config_cache_misses,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct JsonFileDiff {
    path: String,
    operations: Vec<JsonDiffOp>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct JsonDiffOp {
    op: JsonOpKind,
    text: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum JsonOpKind {
    Equal,
    Insert,
    Remove,
}

fn render_json_diff(path: &Path, original: &str, formatted: &str) -> JsonFileDiff {
    let mut operations = Vec::new();

    for change in lines(original, formatted) {
        match change {
            DiffResult::Left(line) => operations.push(JsonDiffOp {
                op: JsonOpKind::Remove,
                text: line.to_string(),
            }),
            DiffResult::Right(line) => operations.push(JsonDiffOp {
                op: JsonOpKind::Insert,
                text: line.to_string(),
            }),
            DiffResult::Both(line, _) => operations.push(JsonDiffOp {
                op: JsonOpKind::Equal,
                text: line.to_string(),
            }),
        }
    }

    JsonFileDiff {
        path: path.display().to_string(),
        operations,
    }
}

fn maybe_emit_stats(options: &FormatOptions, stats: &FormatterRunStats) {
    if options.stats {
        emit_stats(stats);
    }
}

fn emit_stats(stats: &FormatterRunStats) {
    match serde_json::to_string_pretty(stats) {
        Ok(serialized) => println!("{}", serialized),
        Err(error) => eprintln!("Failed to serialize formatter stats: {}", error),
    }
}

fn format_source(input: &str, config: &FormatterConfig) -> String {
    match cst::format_with_cst(input, config) {
        Ok(formatted) => formatted,
        Err(_) => legacy_format_source(input, config),
    }
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

fn legacy_format_source(input: &str, config: &FormatterConfig) -> String {
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

    finalize_output(lines, config)
}

fn finalize_output(mut lines: Vec<FormattedLine>, config: &FormatterConfig) -> String {
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

mod cst {
    use super::{
        count_brace_transitions, count_leading_closing_braces, finalize_output, normalize_spacing,
        FormattedLine, FormatterConfig,
    };
    use spectra_compiler::ast::{
        Block, Enum, Expression, ExpressionKind, Function, ImplBlock, Import, Item, Method, Module,
        Statement, StatementKind, Struct, TraitDeclaration, TraitImpl,
    };
    use spectra_compiler::token::{Keyword, Operator, Token, TokenKind};
    use spectra_compiler::{span::Span, Lexer, Parser};
    use std::collections::HashSet;
    use std::mem;
    use std::ops::Range;

    pub(super) fn format_with_cst(input: &str, config: &FormatterConfig) -> Result<String, ()> {
        let tokens = Lexer::new(input).tokenize().map_err(|_| ())?;
        let parser_tokens = tokens.clone();
        let module = Parser::new(parser_tokens, HashSet::new())
            .parse()
            .map_err(|_| ())?;

        let lines = build_lines(input, &tokens)?;

        let mut formatted = Vec::new();
        let mut indent_level = 0usize;

        for line in lines {
            match line {
                CstLine::Blank => formatted.push(FormattedLine::blank()),
                CstLine::DocComment(text) => {
                    formatted.push(FormattedLine::new(indent_level, text));
                }
                CstLine::Content(elements) => {
                    let raw_line = build_line_text(&elements);
                    if raw_line.trim().is_empty() {
                        formatted.push(FormattedLine::blank());
                        continue;
                    }

                    let normalized = normalize_spacing(&raw_line);
                    let trimmed = normalized.trim_start();
                    let mut dedent = count_leading_closing_braces(trimmed);
                    if dedent > indent_level {
                        dedent = indent_level;
                    }
                    let indent_for_line = indent_level.saturating_sub(dedent);

                    let (opens, closes) = count_brace_transitions(trimmed, dedent);
                    formatted.push(FormattedLine::new(indent_for_line, normalized));
                    indent_level = indent_for_line + opens;
                    indent_level = indent_level.saturating_sub(closes);
                }
            }
        }

        apply_ast_policies(&mut formatted, &module, &tokens, input);

        Ok(finalize_output(formatted, config))
    }

    enum CstLine {
        Blank,
        DocComment(String),
        Content(Vec<LineElement>),
    }

    #[derive(Clone)]
    struct LineToken {
        kind: TokenKind,
        lexeme: String,
    }

    enum LineElement {
        Token(LineToken),
        Comment(String),
    }

    #[derive(Default)]
    struct LineAccumulator {
        elements: Vec<LineElement>,
    }

    impl LineAccumulator {
        fn push_token(&mut self, token: LineToken) {
            self.elements.push(LineElement::Token(token));
        }

        fn push_comment(&mut self, comment: String) {
            self.elements.push(LineElement::Comment(comment));
        }

        fn finish_into(&mut self, lines: &mut Vec<CstLine>) {
            if self.elements.is_empty() {
                return;
            }
            lines.push(CstLine::Content(mem::take(&mut self.elements)));
        }

        fn consume_into_blank(&mut self, lines: &mut Vec<CstLine>) {
            if self.elements.is_empty() {
                lines.push(CstLine::Blank);
            } else {
                lines.push(CstLine::Content(mem::take(&mut self.elements)));
            }
        }
    }

    fn build_lines(source: &str, tokens: &[Token]) -> Result<Vec<CstLine>, ()> {
        let mut lines = Vec::new();
        let mut current = LineAccumulator::default();
        let mut previous_end = 0usize;

        for token in tokens {
            if matches!(token.kind, TokenKind::EndOfFile) {
                break;
            }

            let start = token.span.start;
            let end = token.span.end;

            if start > end || end > source.len() {
                return Err(());
            }

            let trivia = &source[previous_end..start];
            process_trivia(trivia, &mut lines, &mut current);

            let lexeme = source[start..end].to_string();
            current.push_token(LineToken {
                kind: token.kind.clone(),
                lexeme,
            });

            previous_end = end;
        }

        let trailing = &source[previous_end..];
        process_trivia(trailing, &mut lines, &mut current);
        current.finish_into(&mut lines);

        Ok(lines)
    }

    fn process_trivia(text: &str, lines: &mut Vec<CstLine>, current: &mut LineAccumulator) {
        let mut chars = text.chars().peekable();
        let mut suppress_blank_line = false;

        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {}
                '\n' => {
                    if suppress_blank_line {
                        suppress_blank_line = false;
                    } else {
                        current.consume_into_blank(lines);
                    }
                }
                '/' if matches!(chars.peek(), Some('/')) => {
                    chars.next();
                    let is_doc = matches!(chars.peek(), Some('/'));
                    let mut comment = String::from("//");
                    if is_doc {
                        comment.push('/');
                        chars.next();
                    }
                    while let Some(&next) = chars.peek() {
                        if next == '\n' {
                            break;
                        }
                        comment.push(next);
                        chars.next();
                    }
                    let trimmed = comment.trim_end().to_string();
                    if is_doc {
                        current.finish_into(lines);
                        lines.push(CstLine::DocComment(sanitize_doc_comment(&trimmed)));
                        suppress_blank_line = true;
                    } else {
                        current.push_comment(trimmed);
                        suppress_blank_line = false;
                    }
                }
                _ => {}
            }
        }
    }

    fn sanitize_doc_comment(comment: &str) -> String {
        let trimmed = comment.trim_start();
        let without_prefix = trimmed.strip_prefix("///").unwrap_or(trimmed);
        let payload = without_prefix.trim();
        if payload.is_empty() {
            "///".to_string()
        } else {
            format!("/// {}", payload)
        }
    }

    fn build_line_text(elements: &[LineElement]) -> String {
        let mut result = String::new();
        let mut pending_space = false;
        let mut prev_token: Option<&LineToken> = None;

        for (index, element) in elements.iter().enumerate() {
            match element {
                LineElement::Token(token) => {
                    let next_token = next_token(elements, index + 1);
                    apply_token(
                        &mut result,
                        &mut pending_space,
                        token,
                        prev_token,
                        next_token,
                    );
                    prev_token = Some(token);
                }
                LineElement::Comment(comment) => {
                    if !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.push_str(comment);
                    break;
                }
            }
        }

        result.trim_end().to_string()
    }

    fn next_token<'a>(elements: &'a [LineElement], start: usize) -> Option<&'a LineToken> {
        elements
            .get(start..)?
            .iter()
            .find_map(|element| match element {
                LineElement::Token(token) => Some(token),
                _ => None,
            })
    }

    fn apply_token(
        result: &mut String,
        pending_space: &mut bool,
        token: &LineToken,
        prev: Option<&LineToken>,
        next: Option<&LineToken>,
    ) {
        let decision = space_before_decision(token, prev, next);
        let mut insert_space = match decision {
            SpaceDecision::Force => true,
            SpaceDecision::Suppress => false,
            SpaceDecision::Inherit => *pending_space,
        };
        *pending_space = false;

        if disallow_space_before(token) || matches!(decision, SpaceDecision::Suppress) {
            insert_space = false;
            while result.ends_with(' ') {
                result.pop();
            }
        }

        if insert_space && !result.is_empty() && !result.ends_with(' ') {
            result.push(' ');
        }

        result.push_str(&token.lexeme);

        if should_force_space_after(token, prev, next) {
            *pending_space = true;
        }
    }

    #[derive(Clone, Copy)]
    enum SpaceDecision {
        Inherit,
        Force,
        Suppress,
    }

    fn disallow_space_before(token: &LineToken) -> bool {
        matches!(
            token.kind,
            TokenKind::Symbol(ch) if matches!(ch, ',' | ';' | ')' | ']' | '}' | '.' | ':')
        )
    }

    fn space_before_decision(
        token: &LineToken,
        prev: Option<&LineToken>,
        next: Option<&LineToken>,
    ) -> SpaceDecision {
        match &token.kind {
            TokenKind::Symbol('(') => {
                if let Some(prev) = prev {
                    if let TokenKind::Keyword(keyword) = &prev.kind {
                        if keyword_requires_paren_space(keyword) {
                            return SpaceDecision::Force;
                        }
                    }
                    if matches!(
                        prev.kind,
                        TokenKind::Identifier(_)
                            | TokenKind::Number(_)
                            | TokenKind::StringLiteral(_)
                            | TokenKind::Symbol(')' | ']' | '}')
                    ) {
                        return SpaceDecision::Suppress;
                    }
                }
                SpaceDecision::Suppress
            }
            TokenKind::Symbol('[') => SpaceDecision::Suppress,
            TokenKind::Symbol('{') => {
                if prev.is_some() {
                    SpaceDecision::Force
                } else {
                    SpaceDecision::Inherit
                }
            }
            TokenKind::Symbol('-') | TokenKind::Symbol('!') => {
                if is_unary_operator(token, prev) {
                    SpaceDecision::Suppress
                } else {
                    SpaceDecision::Force
                }
            }
            TokenKind::Symbol(ch) if is_binary_symbol(*ch) => SpaceDecision::Force,
            TokenKind::Operator(_) => {
                if is_unary_operator(token, prev) {
                    SpaceDecision::Suppress
                } else {
                    SpaceDecision::Force
                }
            }
            TokenKind::Keyword(_) => {
                if prev.is_some() {
                    SpaceDecision::Force
                } else {
                    SpaceDecision::Inherit
                }
            }
            TokenKind::Identifier(_) | TokenKind::Number(_) | TokenKind::StringLiteral(_) => {
                if let Some(prev_token) = prev {
                    if requires_space_between(prev_token) {
                        SpaceDecision::Force
                    } else {
                        SpaceDecision::Inherit
                    }
                } else {
                    SpaceDecision::Inherit
                }
            }
            TokenKind::Symbol(':') => {
                if let Some(next_token) = next {
                    if matches!(next_token.kind, TokenKind::Symbol(':')) {
                        SpaceDecision::Suppress
                    } else {
                        SpaceDecision::Suppress
                    }
                } else {
                    SpaceDecision::Suppress
                }
            }
            TokenKind::Symbol('.') | TokenKind::Symbol(',') | TokenKind::Symbol(';') => {
                SpaceDecision::Suppress
            }
            TokenKind::Symbol(')') | TokenKind::Symbol(']') | TokenKind::Symbol('}') => {
                SpaceDecision::Suppress
            }
            _ => SpaceDecision::Inherit,
        }
    }

    fn is_binary_symbol(ch: char) -> bool {
        matches!(ch, '=' | '+' | '*' | '/' | '%' | '<' | '>' | '&' | '|')
    }

    fn requires_space_between(token: &LineToken) -> bool {
        matches!(
            token.kind,
            TokenKind::Identifier(_)
                | TokenKind::Number(_)
                | TokenKind::StringLiteral(_)
                | TokenKind::Keyword(_)
                | TokenKind::Symbol(')' | ']' | '}')
        )
    }

    fn should_force_space_after(
        token: &LineToken,
        prev: Option<&LineToken>,
        next: Option<&LineToken>,
    ) -> bool {
        match &token.kind {
            TokenKind::Operator(_) => !is_unary_operator(token, prev),
            TokenKind::Symbol(ch) if is_binary_symbol(*ch) => !is_unary_operator(token, prev),
            TokenKind::Symbol('-') => !is_unary_operator(token, prev),
            TokenKind::Symbol(',') | TokenKind::Symbol(';') => true,
            TokenKind::Symbol(':') => {
                if let Some(next_token) = next {
                    !matches!(next_token.kind, TokenKind::Symbol(':'))
                } else {
                    false
                }
            }
            TokenKind::Keyword(keyword) => keyword_requires_space_after(keyword),
            TokenKind::Symbol('{') => true,
            _ => false,
        }
    }

    fn is_unary_operator(token: &LineToken, prev: Option<&LineToken>) -> bool {
        match &token.kind {
            TokenKind::Symbol('-') | TokenKind::Symbol('!') => previous_allows_unary(prev),
            _ => false,
        }
    }

    fn previous_allows_unary(prev: Option<&LineToken>) -> bool {
        match prev {
            None => true,
            Some(token) => match &token.kind {
                TokenKind::Symbol(ch)
                    if matches!(
                        ch,
                        '(' | '[' | '{' | '=' | ',' | ':' | '+' | '-' | '*' | '/' | '%'
                    ) =>
                {
                    true
                }
                TokenKind::Operator(_) => true,
                TokenKind::Keyword(keyword) => keyword_expects_expression(keyword),
                _ => false,
            },
        }
    }

    fn keyword_requires_paren_space(keyword: &Keyword) -> bool {
        matches!(
            keyword,
            Keyword::If
                | Keyword::While
                | Keyword::For
                | Keyword::Switch
                | Keyword::Match
                | Keyword::Unless
        )
    }

    fn keyword_requires_space_after(keyword: &Keyword) -> bool {
        matches!(
            keyword,
            Keyword::Fn
                | Keyword::Let
                | Keyword::If
                | Keyword::Else
                | Keyword::Elif
                | Keyword::ElseIf
                | Keyword::While
                | Keyword::For
                | Keyword::Match
                | Keyword::Switch
                | Keyword::Unless
                | Keyword::Return
                | Keyword::Struct
                | Keyword::Enum
                | Keyword::Impl
                | Keyword::Trait
                | Keyword::Class
                | Keyword::Pub
                | Keyword::Mut
        )
    }

    fn keyword_expects_expression(keyword: &Keyword) -> bool {
        matches!(
            keyword,
            Keyword::Return
                | Keyword::If
                | Keyword::Else
                | Keyword::Elif
                | Keyword::ElseIf
                | Keyword::While
                | Keyword::For
                | Keyword::Match
                | Keyword::Switch
                | Keyword::Unless
                | Keyword::Case
                | Keyword::Cond
        )
    }

    fn apply_ast_policies(
        lines: &mut [FormattedLine],
        module: &Module,
        tokens: &[Token],
        source: &str,
    ) {
        let line_offsets = compute_line_offsets(source);
        ensure_doc_comment_alignment(lines, module, &line_offsets);
        enforce_match_arm_formatting(lines, module, tokens, source, &line_offsets);
    }

    fn ensure_doc_comment_alignment(
        lines: &mut [FormattedLine],
        module: &Module,
        line_offsets: &[usize],
    ) {
        for item in &module.items {
            adjust_doc_comments_for_span(item_span(item), lines, line_offsets);
            match item {
                Item::Function(func) => {
                    collect_nested_doc_comment_targets_function(func, lines, line_offsets);
                }
                Item::Struct(node) => adjust_doc_comments_for_span(&node.span, lines, line_offsets),
                Item::Enum(node) => adjust_doc_comments_for_span(&node.span, lines, line_offsets),
                Item::Import(node) => adjust_doc_comments_for_span(&node.span, lines, line_offsets),
                Item::Impl(impl_block) => {
                    collect_nested_doc_comment_targets_impl(impl_block, lines, line_offsets);
                }
                Item::Trait(trait_decl) => {
                    collect_nested_doc_comment_targets_trait(trait_decl, lines, line_offsets);
                }
                Item::TraitImpl(trait_impl) => {
                    collect_nested_doc_comment_targets_trait_impl(trait_impl, lines, line_offsets);
                }
            }
        }
    }

    fn enforce_match_arm_formatting(
        lines: &mut [FormattedLine],
        module: &Module,
        tokens: &[Token],
        source: &str,
        line_offsets: &[usize],
    ) {
        let mut spans = Vec::new();
        collect_match_spans_module(module, &mut spans);

        for span in spans {
            rewrite_simple_match_arms(lines, tokens, source, &span, line_offsets);
        }
    }

    fn compute_line_offsets(source: &str) -> Vec<usize> {
        let mut offsets = Vec::new();
        offsets.push(0);
        let mut acc = 0usize;
        for line in source.split_inclusive('\n') {
            acc += line.len();
            offsets.push(acc);
        }
        offsets
    }

    fn line_index_from_offset(offset: usize, offsets: &[usize]) -> Option<usize> {
        match offsets.binary_search(&offset) {
            Ok(index) => Some(index),
            Err(index) => Some(index.saturating_sub(1)),
        }
    }

    fn adjust_doc_comments_for_span(
        span: &Span,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        if let Some(start_line) = line_index_from_offset(span.start, line_offsets) {
            if start_line == 0 || start_line >= lines.len() {
                return;
            }
            let target_indent = lines
                .get(start_line)
                .map(|line| line.indent_level)
                .unwrap_or(0);

            let mut cursor = start_line;
            let mut doc_indices = Vec::new();
            while cursor > 0 {
                let prev_index = cursor - 1;
                if prev_index >= lines.len() {
                    break;
                }
                if lines[prev_index].is_blank {
                    if doc_indices.is_empty() {
                        cursor = prev_index;
                        continue;
                    }
                    break;
                }
                if lines[prev_index].content.trim_start().starts_with("///") {
                    doc_indices.push(prev_index);
                    cursor = prev_index;
                } else {
                    break;
                }
            }

            for index in doc_indices {
                if let Some(line) = lines.get_mut(index) {
                    line.indent_level = target_indent;
                    line.content = sanitize_doc_comment(&line.content);
                }
            }
        }
    }

    fn collect_nested_doc_comment_targets_function(
        function: &Function,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        adjust_doc_comments_for_span(&function.span, lines, line_offsets);
        collect_doc_comments_from_block(&function.body, lines, line_offsets);
    }

    fn collect_nested_doc_comment_targets_impl(
        impl_block: &ImplBlock,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        adjust_doc_comments_for_span(&impl_block.span, lines, line_offsets);
        for method in &impl_block.methods {
            collect_doc_comments_from_method(method, lines, line_offsets);
        }
    }

    fn collect_nested_doc_comment_targets_trait(
        trait_decl: &TraitDeclaration,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        adjust_doc_comments_for_span(&trait_decl.span, lines, line_offsets);
        for method in &trait_decl.methods {
            adjust_doc_comments_for_span(&method.span, lines, line_offsets);
        }
    }

    fn collect_nested_doc_comment_targets_trait_impl(
        trait_impl: &TraitImpl,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        adjust_doc_comments_for_span(&trait_impl.span, lines, line_offsets);
        for method in &trait_impl.methods {
            collect_doc_comments_from_method(method, lines, line_offsets);
        }
    }

    fn collect_doc_comments_from_method(
        method: &Method,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        adjust_doc_comments_for_span(&method.span, lines, line_offsets);
        collect_doc_comments_from_block(&method.body, lines, line_offsets);
    }

    fn collect_doc_comments_from_block(
        block: &Block,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        for statement in &block.statements {
            adjust_doc_comments_for_span(&statement.span, lines, line_offsets);
            collect_doc_comments_from_statement(statement, lines, line_offsets);
        }
    }

    fn collect_doc_comments_from_statement(
        statement: &Statement,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        match &statement.kind {
            StatementKind::Expression(expr) => {
                collect_doc_comments_from_expression(expr, lines, line_offsets)
            }
            StatementKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    collect_doc_comments_from_expression(value, lines, line_offsets);
                }
            }
            StatementKind::Assignment(assign) => {
                collect_doc_comments_from_expression(&assign.value, lines, line_offsets);
            }
            StatementKind::Return(ret) => {
                if let Some(expr) = &ret.value {
                    collect_doc_comments_from_expression(expr, lines, line_offsets);
                }
            }
            StatementKind::While(while_loop) => {
                collect_doc_comments_from_expression(&while_loop.condition, lines, line_offsets);
                collect_doc_comments_from_block(&while_loop.body, lines, line_offsets);
            }
            StatementKind::DoWhile(do_while) => {
                collect_doc_comments_from_block(&do_while.body, lines, line_offsets);
                collect_doc_comments_from_expression(&do_while.condition, lines, line_offsets);
            }
            StatementKind::For(for_loop) => {
                collect_doc_comments_from_expression(&for_loop.iterable, lines, line_offsets);
                collect_doc_comments_from_block(&for_loop.body, lines, line_offsets);
            }
            StatementKind::Loop(loop_stmt) => {
                collect_doc_comments_from_block(&loop_stmt.body, lines, line_offsets);
            }
            StatementKind::Switch(switch_stmt) => {
                collect_doc_comments_from_expression(&switch_stmt.value, lines, line_offsets);
                for case in &switch_stmt.cases {
                    collect_doc_comments_from_block(&case.body, lines, line_offsets);
                }
                if let Some(default) = &switch_stmt.default {
                    collect_doc_comments_from_block(default, lines, line_offsets);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
            StatementKind::IfLet(stmt) => {
                collect_doc_comments_from_expression(&stmt.value, lines, line_offsets);
                collect_doc_comments_from_block(&stmt.then_block, lines, line_offsets);
                if let Some(else_b) = &stmt.else_block {
                    collect_doc_comments_from_block(else_b, lines, line_offsets);
                }
            }
            StatementKind::WhileLet(stmt) => {
                collect_doc_comments_from_expression(&stmt.value, lines, line_offsets);
                collect_doc_comments_from_block(&stmt.body, lines, line_offsets);
            }
        }
    }

    fn collect_doc_comments_from_expression(
        expression: &Expression,
        lines: &mut [FormattedLine],
        line_offsets: &[usize],
    ) {
        adjust_doc_comments_for_span(&expression.span, lines, line_offsets);
        match &expression.kind {
            ExpressionKind::Binary { left, right, .. } => {
                collect_doc_comments_from_expression(left, lines, line_offsets);
                collect_doc_comments_from_expression(right, lines, line_offsets);
            }
            ExpressionKind::Unary { operand, .. } => {
                collect_doc_comments_from_expression(operand, lines, line_offsets);
            }
            ExpressionKind::Call { callee, arguments } => {
                collect_doc_comments_from_expression(callee, lines, line_offsets);
                for arg in arguments {
                    collect_doc_comments_from_expression(arg, lines, line_offsets);
                }
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                collect_doc_comments_from_expression(condition, lines, line_offsets);
                collect_doc_comments_from_block(then_block, lines, line_offsets);
                for (expr, block) in elif_blocks {
                    collect_doc_comments_from_expression(expr, lines, line_offsets);
                    collect_doc_comments_from_block(block, lines, line_offsets);
                }
                if let Some(block) = else_block {
                    collect_doc_comments_from_block(block, lines, line_offsets);
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                collect_doc_comments_from_expression(condition, lines, line_offsets);
                collect_doc_comments_from_block(then_block, lines, line_offsets);
                if let Some(block) = else_block {
                    collect_doc_comments_from_block(block, lines, line_offsets);
                }
            }
            ExpressionKind::ArrayLiteral { elements }
            | ExpressionKind::TupleLiteral { elements } => {
                for element in elements {
                    collect_doc_comments_from_expression(element, lines, line_offsets);
                }
            }
            ExpressionKind::StructLiteral { fields, .. } => {
                for (_, expr) in fields {
                    collect_doc_comments_from_expression(expr, lines, line_offsets);
                }
            }
            ExpressionKind::EnumVariant { data, .. } => {
                if let Some(elements) = data {
                    for expr in elements {
                        collect_doc_comments_from_expression(expr, lines, line_offsets);
                    }
                }
            }
            ExpressionKind::Match { scrutinee, arms } => {
                collect_doc_comments_from_expression(scrutinee, lines, line_offsets);
                for arm in arms {
                    collect_doc_comments_from_expression(&arm.body, lines, line_offsets);
                }
            }
            ExpressionKind::MethodCall {
                object, arguments, ..
            } => {
                collect_doc_comments_from_expression(object, lines, line_offsets);
                for arg in arguments {
                    collect_doc_comments_from_expression(arg, lines, line_offsets);
                }
            }
            ExpressionKind::Grouping(expr)
            | ExpressionKind::IndexAccess { array: expr, .. }
            | ExpressionKind::FieldAccess { object: expr, .. }
            | ExpressionKind::TupleAccess { tuple: expr, .. } => {
                collect_doc_comments_from_expression(expr, lines, line_offsets);
            }
            ExpressionKind::NumberLiteral(_)
            | ExpressionKind::StringLiteral(_)
            | ExpressionKind::BoolLiteral(_)
            | ExpressionKind::Identifier(_) => {}
            ExpressionKind::CharLiteral(_) => {}
            ExpressionKind::FString(parts) => {
                for part in parts {
                    if let spectra_compiler::ast::FStringPart::Interpolated(expr) = part {
                        collect_doc_comments_from_expression(expr, lines, line_offsets);
                    }
                }
            }
            ExpressionKind::Lambda { body, .. } => {
                collect_doc_comments_from_expression(body, lines, line_offsets);
            }
            ExpressionKind::Try(inner) => {
                collect_doc_comments_from_expression(inner, lines, line_offsets);
            }
            ExpressionKind::Range { start, end, .. } => {
                collect_doc_comments_from_expression(start, lines, line_offsets);
                collect_doc_comments_from_expression(end, lines, line_offsets);
            }
            ExpressionKind::Block(block) => {
                for stmt in &block.statements {
                    collect_doc_comments_from_statement(stmt, lines, line_offsets);
                }
            }
        }
    }

    fn collect_match_spans_module(module: &Module, spans: &mut Vec<Span>) {
        for item in &module.items {
            match item {
                Item::Function(function) => collect_match_spans_block(&function.body, spans),
                Item::Impl(impl_block) => collect_match_spans_impl(impl_block, spans),
                Item::Trait(trait_decl) => collect_match_spans_trait(trait_decl, spans),
                Item::TraitImpl(trait_impl) => collect_match_spans_trait_impl(trait_impl, spans),
                Item::Struct(_) | Item::Enum(_) | Item::Import(_) => {}
            }
        }
    }

    fn collect_match_spans_impl(impl_block: &ImplBlock, spans: &mut Vec<Span>) {
        for method in &impl_block.methods {
            collect_match_spans_block(&method.body, spans);
        }
    }

    fn collect_match_spans_trait(trait_decl: &TraitDeclaration, spans: &mut Vec<Span>) {
        for method in &trait_decl.methods {
            if let Some(body) = &method.body {
                collect_match_spans_block(body, spans);
            }
        }
    }

    fn collect_match_spans_trait_impl(trait_impl: &TraitImpl, spans: &mut Vec<Span>) {
        for method in &trait_impl.methods {
            collect_match_spans_block(&method.body, spans);
        }
    }

    fn collect_match_spans_block(block: &Block, spans: &mut Vec<Span>) {
        for statement in &block.statements {
            collect_match_spans_statement(statement, spans);
        }
    }

    fn collect_match_spans_statement(statement: &Statement, spans: &mut Vec<Span>) {
        match &statement.kind {
            StatementKind::Expression(expr) => collect_match_spans_expression(expr, spans),
            StatementKind::Let(let_stmt) => {
                if let Some(value) = &let_stmt.value {
                    collect_match_spans_expression(value, spans);
                }
            }
            StatementKind::Assignment(assign) => {
                collect_match_spans_expression(&assign.value, spans);
            }
            StatementKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    collect_match_spans_expression(value, spans);
                }
            }
            StatementKind::While(while_loop) => {
                collect_match_spans_expression(&while_loop.condition, spans);
                collect_match_spans_block(&while_loop.body, spans);
            }
            StatementKind::DoWhile(do_while) => {
                collect_match_spans_block(&do_while.body, spans);
                collect_match_spans_expression(&do_while.condition, spans);
            }
            StatementKind::For(for_loop) => {
                collect_match_spans_expression(&for_loop.iterable, spans);
                collect_match_spans_block(&for_loop.body, spans);
            }
            StatementKind::Loop(loop_stmt) => collect_match_spans_block(&loop_stmt.body, spans),
            StatementKind::Switch(switch_stmt) => {
                collect_match_spans_expression(&switch_stmt.value, spans);
                for case in &switch_stmt.cases {
                    collect_match_spans_block(&case.body, spans);
                }
                if let Some(default) = &switch_stmt.default {
                    collect_match_spans_block(default, spans);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
            StatementKind::IfLet(stmt) => {
                collect_match_spans_expression(&stmt.value, spans);
                collect_match_spans_block(&stmt.then_block, spans);
                if let Some(else_b) = &stmt.else_block {
                    collect_match_spans_block(else_b, spans);
                }
            }
            StatementKind::WhileLet(stmt) => {
                collect_match_spans_expression(&stmt.value, spans);
                collect_match_spans_block(&stmt.body, spans);
            }
        }
    }

    fn collect_match_spans_expression(expr: &Expression, spans: &mut Vec<Span>) {
        match &expr.kind {
            ExpressionKind::Binary { left, right, .. } => {
                collect_match_spans_expression(left, spans);
                collect_match_spans_expression(right, spans);
            }
            ExpressionKind::Unary { operand, .. } => collect_match_spans_expression(operand, spans),
            ExpressionKind::Call { callee, arguments } => {
                collect_match_spans_expression(callee, spans);
                for arg in arguments {
                    collect_match_spans_expression(arg, spans);
                }
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                collect_match_spans_expression(condition, spans);
                collect_match_spans_block(then_block, spans);
                for (expr, block) in elif_blocks {
                    collect_match_spans_expression(expr, spans);
                    collect_match_spans_block(block, spans);
                }
                if let Some(block) = else_block {
                    collect_match_spans_block(block, spans);
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                collect_match_spans_expression(condition, spans);
                collect_match_spans_block(then_block, spans);
                if let Some(block) = else_block {
                    collect_match_spans_block(block, spans);
                }
            }
            ExpressionKind::ArrayLiteral { elements }
            | ExpressionKind::TupleLiteral { elements } => {
                for element in elements {
                    collect_match_spans_expression(element, spans);
                }
            }
            ExpressionKind::StructLiteral { fields, .. } => {
                for (_, expr) in fields {
                    collect_match_spans_expression(expr, spans);
                }
            }
            ExpressionKind::EnumVariant { data, .. } => {
                if let Some(elements) = data {
                    for expr in elements {
                        collect_match_spans_expression(expr, spans);
                    }
                }
            }
            ExpressionKind::Match { scrutinee, arms } => {
                spans.push(expr.span);
                collect_match_spans_expression(scrutinee, spans);
                for arm in arms {
                    collect_match_spans_expression(&arm.body, spans);
                }
            }
            ExpressionKind::MethodCall {
                object, arguments, ..
            } => {
                collect_match_spans_expression(object, spans);
                for arg in arguments {
                    collect_match_spans_expression(arg, spans);
                }
            }
            ExpressionKind::Grouping(inner)
            | ExpressionKind::IndexAccess { array: inner, .. }
            | ExpressionKind::FieldAccess { object: inner, .. }
            | ExpressionKind::TupleAccess { tuple: inner, .. } => {
                collect_match_spans_expression(inner, spans);
            }
            ExpressionKind::NumberLiteral(_)
            | ExpressionKind::StringLiteral(_)
            | ExpressionKind::BoolLiteral(_)
            | ExpressionKind::Identifier(_) => {}
            ExpressionKind::CharLiteral(_) => {}
            ExpressionKind::FString(parts) => {
                for part in parts {
                    if let spectra_compiler::ast::FStringPart::Interpolated(expr) = part {
                        collect_match_spans_expression(expr, spans);
                    }
                }
            }
            ExpressionKind::Lambda { body, .. } => {
                collect_match_spans_expression(body, spans);
            }
            ExpressionKind::Try(inner) => {
                collect_match_spans_expression(inner, spans);
            }
            ExpressionKind::Range { start, end, .. } => {
                collect_match_spans_expression(start, spans);
                collect_match_spans_expression(end, spans);
            }
            ExpressionKind::Block(block) => {
                collect_match_spans_block(block, spans);
            }
        }
    }

    fn rewrite_simple_match_arms(
        lines: &mut [FormattedLine],
        tokens: &[Token],
        source: &str,
        span: &Span,
        line_offsets: &[usize],
    ) {
        let (start_index, end_index) = match token_range_for_span(tokens, span) {
            Some(range) => range,
            None => return,
        };

        let mut brace_depth = 0usize;
        let mut paren_depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut in_match_body = false;

        let mut current_arm_start = None;
        let mut current_arm: Option<(Range<usize>, usize)> = None;
        let mut last_token_end = span.start;

        for token in &tokens[start_index..=end_index] {
            match &token.kind {
                TokenKind::Symbol('{') => {
                    if in_match_body {
                        brace_depth += 1;
                    } else {
                        in_match_body = true;
                        brace_depth = 1;
                        current_arm_start = Some(token.span.end);
                    }
                }
                TokenKind::Symbol('}') => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                    if in_match_body && brace_depth == 0 {
                        if let Some((pattern_range, arrow_end)) = current_arm.take() {
                            finalize_arm(
                                lines,
                                source,
                                &pattern_range,
                                arrow_end,
                                token.span.start,
                                false,
                                line_offsets,
                            );
                        }
                        break;
                    }
                }
                TokenKind::Symbol('(') => paren_depth += 1,
                TokenKind::Symbol(')') => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                TokenKind::Symbol('[') => bracket_depth += 1,
                TokenKind::Symbol(']') => {
                    if bracket_depth > 0 {
                        bracket_depth -= 1;
                    }
                }
                TokenKind::Operator(Operator::FatArrow)
                    if in_match_body
                        && brace_depth == 1
                        && paren_depth == 0
                        && bracket_depth == 0 =>
                {
                    let start = current_arm_start.unwrap_or(last_token_end);
                    let pattern_range = Range {
                        start,
                        end: token.span.start,
                    };
                    current_arm = Some((pattern_range, token.span.end));
                }
                TokenKind::Symbol(',')
                    if in_match_body
                        && brace_depth == 1
                        && paren_depth == 0
                        && bracket_depth == 0 =>
                {
                    if let Some((pattern_range, arrow_end)) = current_arm.take() {
                        finalize_arm(
                            lines,
                            source,
                            &pattern_range,
                            arrow_end,
                            token.span.start,
                            true,
                            line_offsets,
                        );
                        current_arm_start = Some(token.span.end);
                    }
                }
                _ => {}
            }

            last_token_end = token.span.end;
        }
    }

    fn token_range_for_span(tokens: &[Token], span: &Span) -> Option<(usize, usize)> {
        let start_index = tokens
            .iter()
            .position(|token| token.span.end > span.start)?;
        let mut end_index = start_index;
        while end_index < tokens.len() && tokens[end_index].span.start < span.end {
            end_index += 1;
        }
        if end_index == 0 {
            return None;
        }
        Some((start_index, end_index.saturating_sub(1)))
    }

    fn finalize_arm(
        lines: &mut [FormattedLine],
        source: &str,
        pattern_range: &Range<usize>,
        arrow_end: usize,
        body_end_start: usize,
        had_comma: bool,
        line_offsets: &[usize],
    ) {
        let pattern_range = trim_range(source, pattern_range.clone());
        if pattern_range.start >= pattern_range.end {
            return;
        }
        let body_range = trim_range(
            source,
            Range {
                start: arrow_end,
                end: body_end_start,
            },
        );
        if body_range.start >= body_range.end {
            return;
        }

        let pattern_text = &source[pattern_range.clone()];
        let body_text = &source[body_range.clone()];
        if pattern_text.contains('\n') || body_text.contains('\n') {
            return;
        }

        let arrow_line = match line_index_from_offset(pattern_range.end, line_offsets) {
            Some(index) if index < lines.len() => index,
            _ => return,
        };

        let mut rebuilt = String::new();
        rebuilt.push_str(pattern_text.trim());
        rebuilt.push_str(" => ");
        rebuilt.push_str(body_text.trim());
        if had_comma && !rebuilt.trim_end().ends_with(',') {
            rebuilt.push(',');
        } else if !had_comma && rebuilt.trim_end().ends_with(',') {
            let mut trimmed = rebuilt
                .trim_end_matches(|ch: char| ch == ',' || ch.is_whitespace())
                .to_string();
            if pattern_text.trim().ends_with(',') {
                trimmed.push(',');
            }
            rebuilt = trimmed;
        }

        if let Some(target) = lines.get_mut(arrow_line) {
            if !target.content.contains("=>") {
                return;
            }
            target.content = rebuilt;
        }
    }

    fn trim_range(source: &str, range: Range<usize>) -> Range<usize> {
        if range.start >= range.end {
            return range;
        }
        let slice = &source[range.clone()];
        let trimmed_start = slice.trim_start();
        let leading = slice.len().saturating_sub(trimmed_start.len());
        let trimmed = trimmed_start.trim_end();
        let trailing = trimmed_start.len().saturating_sub(trimmed.len());
        Range {
            start: range.start + leading,
            end: range.end.saturating_sub(trailing),
        }
    }

    fn item_span(item: &Item) -> &Span {
        match item {
            Item::Import(Import { span, .. }) => span,
            Item::Function(Function { span, .. }) => span,
            Item::Struct(Struct { span, .. }) => span,
            Item::Enum(Enum { span, .. }) => span,
            Item::Impl(ImplBlock { span, .. }) => span,
            Item::Trait(TraitDeclaration { span, .. }) => span,
            Item::TraitImpl(TraitImpl { span, .. }) => span,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

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

    #[test]
    fn keeps_else_if_spacing() {
        let input =
            "fn flag(){\nif ready {\nreturn;\n}else if pending {\nreturn;\n}else{\nreturn;\n}\n}\n";
        let expected =
            "fn flag() {\n    if ready {\n        return;\n    } else if pending {\n        return;\n    } else {\n        return;\n    }\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn preserves_double_colon_compact() {
        let input = "fn main(){\nlet value=Namespace::member();\nreturn value;\n}\n";
        let expected = "fn main() {\n    let value = Namespace::member();\n    return value;\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn keeps_unary_minus_tight() {
        let input =
            "fn eval(flag: bool){\nif flag {\nreturn -value;\n}else{\nreturn !flag;\n}\n}\n";
        let expected =
            "fn eval(flag: bool) {\n    if flag {\n        return -value;\n    } else {\n        return !flag;\n    }\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn formats_doc_comments_adjacent_to_function() {
        let input = "///short\nfn demo(){\nreturn 42;\n}\n";
        let expected = "/// short\nfn demo() {\n    return 42;\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn normalizes_simple_match_arms() {
        let input = "fn classify(value){\nmatch value {\nAlpha=>1,\nBeta=>2,\nGamma=>3\n}\n}\n";
        let expected = "fn classify(value) {\n    match value {\n        Alpha => 1,\n        Beta => 2,\n        Gamma => 3\n    }\n}\n";
        assert_eq!(format_source(input, &FormatterConfig::default()), expected);
    }

    #[test]
    fn render_diff_reports_changes() {
        let original = "fn demo() {\n    return 0;\n}\n";
        let formatted = "fn demo() {\n    return 1;\n}\n";
        let diff = super::render_diff(Path::new("demo.spectra"), original, formatted);
        assert!(diff.contains("diff --spectra demo.spectra"));
        assert!(diff.contains("--- original"));
        assert!(diff.contains("+++ formatted"));
        assert!(diff.contains("-    return 0;"));
        assert!(diff.contains("+    return 1;"));
    }

    #[test]
    fn render_json_diff_reports_operations() {
        let original = "fn demo() {\n    return 0;\n}\n";
        let formatted = "fn demo() {\n    return 1;\n}\n";
        let file_diff = super::render_json_diff(Path::new("demo.spectra"), original, formatted);
        assert_eq!(file_diff.path, "demo.spectra");
        assert!(file_diff
            .operations
            .iter()
            .any(|op| matches!(op.op, super::JsonOpKind::Remove) && op.text.contains("return 0")));
        assert!(file_diff
            .operations
            .iter()
            .any(|op| matches!(op.op, super::JsonOpKind::Insert) && op.text.contains("return 1")));
    }

    #[test]
    fn formatter_run_stats_check_mode_reports_zero_updates() {
        let mut config_stats = super::ConfigStats::default();
        config_stats.record_miss();
        let stats = super::FormatterRunStats::new(2, 1, true, &config_stats);
        assert_eq!(stats.processed, 2);
        assert_eq!(stats.changed, 1);
        assert_eq!(stats.updated, 0);
        assert_eq!(stats.unchanged, 1);
        assert_eq!(stats.mode, super::FormatterMode::Check);
        assert_eq!(stats.config_cache_lookups, 1);
        assert_eq!(stats.config_cache_hits, 0);
        assert_eq!(stats.config_cache_misses, 1);
    }

    #[test]
    fn json_summary_reflects_run_stats() {
        let mut config_stats = super::ConfigStats::default();
        config_stats.record_hit();
        let stats = super::FormatterRunStats::new(3, 2, false, &config_stats);
        let summary = super::JsonSummary::from_stats(&stats);
        assert_eq!(summary.processed, 3);
        assert_eq!(summary.changed, 2);
        assert_eq!(summary.updated, 2);
        assert_eq!(summary.unchanged, 1);
        assert_eq!(summary.mode, super::FormatterMode::Write);
        assert_eq!(stats.config_cache_hits, 1);
        assert_eq!(summary.config_cache_lookups, 1);
        assert_eq!(summary.config_cache_hits, 1);
        assert_eq!(summary.config_cache_misses, 0);
    }
}
