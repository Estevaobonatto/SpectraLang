mod compiler_integration;
mod config;
mod discovery;
mod formatter;
mod linker;
mod project;
mod runtime_lib;

use compiler_integration::{
    forward_program_args, take_last_exec_exit, ModulePipelineSummary, SpectraCompiler,
};
use formatter::{run as run_formatter, ExplainMode, FormatOptions};
use project::ProjectPlan;
use serde::{Deserialize, Serialize};
use spectra_compiler::{
    error::CompilerError, lint::LintDiagnostic, span::Span, CompilationOptions, LintOptions,
    LintRule,
};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{env, fs, process};

const KNOWN_EXPERIMENTAL_FEATURES: &[&str] = &["switch", "unless", "do-while", "loop"];

#[repr(i32)]
#[derive(Copy, Clone, Debug)]
enum ExitCode {
    Success = 0,
    Usage = 64,
    CompilationFailed = 65,
    IoError = 74,
}

impl ExitCode {
    fn as_i32(self) -> i32 {
        self as i32
    }
}

#[derive(Debug)]
struct CliError {
    message: String,
    code: ExitCode,
}

impl CliError {
    fn new(message: impl Into<String>, code: ExitCode) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }

    fn usage(message: impl Into<String>) -> Self {
        Self::new(message, ExitCode::Usage)
    }

    fn compilation(message: impl Into<String>) -> Self {
        Self::new(message, ExitCode::CompilationFailed)
    }

    fn io(message: impl Into<String>) -> Self {
        Self::new(message, ExitCode::IoError)
    }
}

type CliResult<T> = Result<T, CliError>;

fn log_error(message: &str) {
    for (index, line) in message.lines().enumerate() {
        if index == 0 {
            eprintln!("error: {}", line);
        } else if line.is_empty() {
            eprintln!();
        } else {
            eprintln!("       {}", line);
        }
    }
}

#[derive(Debug)]
struct CliInvocation {
    entries: Vec<PathBuf>,
    options: CompilationOptions,
    show_pipeline_summary: bool,
    verbose: bool,
    json_output: bool,
    /// When `Some(path)`, emit a native object file at `path` instead of / in addition to JIT.
    emit_object: Option<PathBuf>,
    /// When `Some(path)`, compile to a native executable at `path`.
    emit_exe: Option<PathBuf>,
    /// Arguments forwarded to the Spectra program when running via JIT (`run` command).
    /// These are accessible through `std.env.env_arg` / `std.env.env_args_count`.
    program_args: Vec<String>,
}

#[derive(Debug)]
struct ReplOptions {
    base_options: CompilationOptions,
    preload: Vec<PathBuf>,
    autorun: bool,
    show_pipeline_summary: bool,
    verbose: bool,
    json_output: bool,
}

#[derive(Debug)]
struct NewProjectOptions {
    path: PathBuf,
    force: bool,
}

#[derive(Debug)]
enum CliAction {
    Help(HelpTopic),
    ListExperimental,
    Build {
        kind: BuildCommand,
        invocation: CliInvocation,
    },
    Repl(ReplOptions),
    NewProject(NewProjectOptions),
    Format(FormatOptions),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum HelpTopic {
    Global,
    Build(BuildCommand),
    Repl,
    NewProject,
    Format,
    Lint,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BuildCommand {
    Compile,
    Check,
    Run,
    Lint,
}

impl BuildCommand {
    fn name(self) -> &'static str {
        match self {
            BuildCommand::Compile => "compile",
            BuildCommand::Check => "check",
            BuildCommand::Run => "run",
            BuildCommand::Lint => "lint",
        }
    }

    fn description(self) -> &'static str {
        match self {
            BuildCommand::Compile => "Compile Spectra modules (default).",
            BuildCommand::Check => "Type-check modules and report diagnostics without executing.",
            BuildCommand::Run => "Compile modules and execute the entry point via JIT.",
            BuildCommand::Lint => "Run lint checks and report warnings or denied rules.",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            BuildCommand::Compile => "All files compiled successfully!",
            BuildCommand::Check => "Check completed. No errors detected.",
            BuildCommand::Run => "Compilation and execution finished successfully.",
            BuildCommand::Lint => "Lint checks completed without findings.",
        }
    }

    fn module_verb(self) -> &'static str {
        match self {
            BuildCommand::Check => "Checking",
            BuildCommand::Lint => "Linting",
            BuildCommand::Compile | BuildCommand::Run => "Compiling",
        }
    }

    fn module_success_verb(self) -> &'static str {
        match self {
            BuildCommand::Check => "checked",
            BuildCommand::Lint => "linted",
            BuildCommand::Compile | BuildCommand::Run => "compiled",
        }
    }
}

fn main() {
    let exit_code = match run_cli() {
        Ok(()) => ExitCode::Success,
        Err(error) => {
            log_error(&error.message);
            error.code
        }
    };

    process::exit(exit_code.as_i32());
}

fn run_cli() -> CliResult<()> {
    let action = parse_cli()?;
    execute_action(action)
}

fn execute_action(action: CliAction) -> CliResult<()> {
    match action {
        CliAction::Help(topic) => {
            match topic {
                HelpTopic::Global => print_global_help(),
                HelpTopic::Build(command) => print_build_help(command),
                HelpTopic::Repl => print_repl_help(),
                HelpTopic::NewProject => print_new_help(),
                HelpTopic::Format => print_format_help(),
                HelpTopic::Lint => print_lint_help(),
            }
            Ok(())
        }
        CliAction::ListExperimental => {
            print_experimental_features();
            Ok(())
        }
        CliAction::Build { kind, invocation } => execute_build_command(kind, invocation),
        CliAction::Repl(options) => execute_repl(options),
        CliAction::NewProject(options) => execute_new_project(options),
        CliAction::Format(options) => execute_format(options),
    }
}

fn parse_cli() -> CliResult<CliAction> {
    let mut args = env::args().skip(1).peekable();

    if args.peek().is_none() {
        return Err(usage_error("No command or input files provided."));
    }

    match args.peek().map(|value| value.as_str()) {
        Some("--help") | Some("-h") => {
            args.next();
            return Ok(CliAction::Help(HelpTopic::Global));
        }
        Some("help") => {
            args.next();
            if let Some(target) = args.next() {
                return match target.as_str() {
                    "new" | "new-project" => Ok(CliAction::Help(HelpTopic::NewProject)),
                    "repl" => Ok(CliAction::Help(HelpTopic::Repl)),
                    "fmt" | "format" => Ok(CliAction::Help(HelpTopic::Format)),
                    "lint" => Ok(CliAction::Help(HelpTopic::Lint)),
                    other => {
                        if let Some(kind) = parse_build_command_name(other) {
                            Ok(CliAction::Help(HelpTopic::Build(kind)))
                        } else {
                            Err(usage_error(&format!("Unknown command '{}'.", other)))
                        }
                    }
                };
            } else {
                return Ok(CliAction::Help(HelpTopic::Global));
            }
        }
        Some("--list-experimental") => {
            args.next();
            if args.peek().is_some() {
                return Err(usage_error("--list-experimental must be used on its own."));
            }
            return Ok(CliAction::ListExperimental);
        }
        Some("repl") => {
            args.next();
            if let Some(flag) = args.peek() {
                if matches!(flag.as_str(), "--help" | "-h") {
                    args.next();
                    return Ok(CliAction::Help(HelpTopic::Repl));
                }
            }

            let options = parse_repl_invocation(&mut args)?;
            return Ok(CliAction::Repl(options));
        }
        Some("new") | Some("new-project") => {
            args.next();
            if let Some(flag) = args.peek() {
                if matches!(flag.as_str(), "--help" | "-h") {
                    args.next();
                    return Ok(CliAction::Help(HelpTopic::NewProject));
                }
            }

            let options = parse_new_project_invocation(&mut args)?;
            return Ok(CliAction::NewProject(options));
        }
        Some("fmt") | Some("format") => {
            args.next();
            if let Some(flag) = args.peek() {
                if matches!(flag.as_str(), "--help" | "-h") {
                    args.next();
                    return Ok(CliAction::Help(HelpTopic::Format));
                }
            }

            let options = parse_format_invocation(&mut args)?;
            return Ok(CliAction::Format(options));
        }
        Some("lint") => {
            args.next();
            if let Some(flag) = args.peek() {
                if matches!(flag.as_str(), "--help" | "-h") {
                    args.next();
                    return Ok(CliAction::Help(HelpTopic::Lint));
                }
            }

            let invocation = parse_compilation_invocation(&mut args, BuildCommand::Lint, true)?;
            return Ok(CliAction::Build {
                kind: BuildCommand::Lint,
                invocation,
            });
        }
        _ => {}
    }

    let mut command = BuildCommand::Compile;

    if let Some(value) = args.peek() {
        if !value.starts_with('-') {
            if let Some(kind) = parse_build_command_name(value) {
                command = kind;
                args.next();
            }
        }
    }

    if let Some(flag) = args.peek() {
        if matches!(flag.as_str(), "--help" | "-h") {
            args.next();
            return Ok(CliAction::Help(HelpTopic::Build(command)));
        }
    }

    let invocation =
        parse_compilation_invocation(&mut args, command, matches!(command, BuildCommand::Lint))?;

    Ok(CliAction::Build {
        kind: command,
        invocation,
    })
}

fn parse_build_command_name(value: &str) -> Option<BuildCommand> {
    match value {
        "compile" | "build" => Some(BuildCommand::Compile),
        "check" => Some(BuildCommand::Check),
        "run" => Some(BuildCommand::Run),
        "lint" => Some(BuildCommand::Lint),
        _ => None,
    }
}

fn parse_compilation_invocation<I>(
    args: &mut std::iter::Peekable<I>,
    command: BuildCommand,
    lint_default: bool,
) -> CliResult<CliInvocation>
where
    I: Iterator<Item = String>,
{
    let mut options = CompilationOptions::default();
    let mut lint_enabled_cli = lint_default;
    let mut lint_allow_cli: Vec<LintRule> = Vec::new();
    let mut lint_deny_cli: Vec<LintRule> = Vec::new();
    let mut entries = Vec::new();
    let mut show_pipeline_summary = false;
    let mut verbose = false;
    let mut json_output = false;
    let mut emit_object: Option<PathBuf> = None;
    let mut emit_exe: Option<PathBuf> = None;
    let mut program_args: Vec<String> = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" => {
                // For `run`: everything after `--` is forwarded as program arguments.
                // For other commands: treat remaining tokens as additional source files
                // (preserves previous behaviour for `compile`, `check`, `lint`).
                let remaining: Vec<String> = args.by_ref().collect();
                if command == BuildCommand::Run {
                    program_args.extend(remaining);
                } else {
                    entries.extend(remaining.into_iter().map(PathBuf::from));
                }
                break;
            }
            "--dump-ast" => options.dump_ast = true,
            "--dump-ir" => options.dump_ir = true,
            "--timings" | "-T" => {
                options.collect_metrics = true;
                show_pipeline_summary = true;
            }
            "--no-optimize" | "-O0" => {
                options.optimize = false;
                options.opt_level = 0;
            }
            "-O1" => {
                options.optimize = true;
                options.opt_level = 1;
            }
            "-O2" => {
                options.optimize = true;
                options.opt_level = 2;
            }
            "-O3" => {
                options.optimize = true;
                options.opt_level = 3;
            }
            "--verbose" | "-v" => verbose = true,
            "--run" | "-r" => {
                if command == BuildCommand::Check {
                    return Err(usage_error(
                        "'--run' cannot be used with the 'check' command.",
                    ));
                }
                options.run_jit = true;
            }
            "--summary" | "--pipeline-summary" => {
                options.collect_metrics = true;
                show_pipeline_summary = true;
            }
            "--enable-experimental" => {
                if let Some(feature) = args.next() {
                    options.experimental_features.insert(feature);
                } else {
                    return Err(usage_error(
                        "Missing feature name after '--enable-experimental'.",
                    ));
                }
            }
            "--list-experimental" => {
                return Err(usage_error(
                    "--list-experimental must appear before any command.",
                ));
            }
            flag if flag.starts_with("--allow=") => {
                let value = flag.trim_start_matches("--allow=");
                let rule = parse_lint_rule_cli(value)?;
                lint_allow_cli.push(rule);
            }
            flag if flag.starts_with("--deny=") => {
                let value = flag.trim_start_matches("--deny=");
                let rule = parse_lint_rule_cli(value)?;
                lint_deny_cli.push(rule);
            }
            "--lint" => {
                lint_enabled_cli = true;
            }
            "--allow" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage_error("Missing rule name after '--allow'."))?;
                let rule = parse_lint_rule_cli(&value)?;
                lint_allow_cli.push(rule);
            }
            "--deny" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage_error("Missing rule name after '--deny'."))?;
                let rule = parse_lint_rule_cli(&value)?;
                lint_deny_cli.push(rule);
            }
            "--json" => {
                if command != BuildCommand::Lint {
                    return Err(usage_error(
                        "'--json' is only supported with the 'lint' command.",
                    ));
                }
                json_output = true;
            }
            "--emit-object" | "-o" => {
                let path = args
                    .next()
                    .ok_or_else(|| usage_error("Missing output path after '--emit-object'."))?;
                emit_object = Some(PathBuf::from(path));
            }
            "--emit-exe" | "-e" => {
                let path = args
                    .next()
                    .ok_or_else(|| usage_error("Missing output path after '--emit-exe'."))?;
                emit_exe = Some(PathBuf::from(path));
            }
            flag if flag.starts_with('-') => {
                return Err(usage_error(&format!("Unknown option: {}", flag)));
            }
            _ => entries.push(PathBuf::from(arg)),
        }
    }

    if entries.is_empty() {
        return Err(usage_error("No source files or directories were provided."));
    }

    match command {
        BuildCommand::Run => options.run_jit = true,
        BuildCommand::Check | BuildCommand::Lint => options.run_jit = false,
        BuildCommand::Compile => {}
    }

    configure_lint_options(
        &mut options,
        &entries,
        lint_enabled_cli,
        &lint_allow_cli,
        &lint_deny_cli,
    )?;

    Ok(CliInvocation {
        entries,
        options,
        show_pipeline_summary,
        verbose,
        json_output,
        emit_object,
        emit_exe,
        program_args,
    })
}

#[derive(Debug, Deserialize, Default)]
struct ManifestLintSection {
    enabled: Option<bool>,
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SpectraManifest {
    #[serde(default)]
    lint: Option<ManifestLintSection>,
}

fn parse_raw_lint_rule(value: &str) -> Result<LintRule, String> {
    LintRule::from_str(value).map_err(|_| {
        format!(
            "Unknown lint rule '{}' (valid rules: {}).",
            value,
            lint_rule_list()
        )
    })
}

fn parse_lint_rule_cli(value: &str) -> CliResult<LintRule> {
    parse_raw_lint_rule(value).map_err(|message| usage_error(&message))
}

fn parse_lint_rule_config(value: &str, path: &Path) -> CliResult<LintRule> {
    parse_raw_lint_rule(value)
        .map_err(|message| CliError::usage(format!("{} (found in '{}').", message, path.display())))
}

fn lint_rule_list() -> String {
    LintRule::all()
        .iter()
        .map(LintRule::code)
        .collect::<Vec<_>>()
        .join(", ")
}

fn configure_lint_options(
    options: &mut CompilationOptions,
    entries: &[PathBuf],
    lint_enabled_cli: bool,
    cli_allow: &[LintRule],
    cli_deny: &[LintRule],
) -> CliResult<()> {
    let manifest_path = locate_manifest(entries)?;

    let mut manifest_enabled = None;
    let mut manifest_allow: Vec<LintRule> = Vec::new();
    let mut manifest_deny: Vec<LintRule> = Vec::new();

    if let Some(path) = &manifest_path {
        let contents = fs::read_to_string(path).map_err(|error| {
            CliError::io(format!("Failed to read '{}': {}", path.display(), error))
        })?;

        let manifest: SpectraManifest = toml::from_str(&contents).map_err(|error| {
            CliError::io(format!("Failed to parse '{}': {}", path.display(), error))
        })?;

        if let Some(lint) = manifest.lint {
            manifest_enabled = lint.enabled;
            for rule in lint.allow {
                manifest_allow.push(parse_lint_rule_config(&rule, path)?);
            }
            for rule in lint.deny {
                manifest_deny.push(parse_lint_rule_config(&rule, path)?);
            }
        }
    }

    let mut enable_lints = lint_enabled_cli;
    if let Some(flag) = manifest_enabled {
        enable_lints = flag;
    }
    if lint_enabled_cli {
        enable_lints = true;
    }

    if enable_lints {
        options.lint = LintOptions::all();
    } else {
        options.lint = LintOptions::disabled();
    }

    for rule in manifest_allow {
        options.lint.disable_rule(rule);
    }
    for &rule in cli_allow {
        options.lint.disable_rule(rule);
    }

    for rule in manifest_deny {
        options.lint.deny_rule(rule);
    }
    for &rule in cli_deny {
        options.lint.deny_rule(rule);
    }

    Ok(())
}

fn locate_manifest(entries: &[PathBuf]) -> CliResult<Option<PathBuf>> {
    for entry in entries {
        let metadata = fs::metadata(entry).map_err(|error| {
            CliError::io(format!(
                "Failed to inspect '{}': {}",
                entry.display(),
                error
            ))
        })?;

        let mut current = if metadata.is_dir() {
            Some(entry.clone())
        } else {
            entry.parent().map(Path::to_path_buf)
        };

        while let Some(dir) = current {
            let candidate = dir.join("Spectra.toml");
            if candidate.is_file() {
                let canonical = fs::canonicalize(&candidate).map_err(|error| {
                    CliError::io(format!(
                        "Failed to resolve configuration '{}': {}",
                        candidate.display(),
                        error
                    ))
                })?;
                return Ok(Some(canonical));
            }
            current = dir.parent().map(Path::to_path_buf);
        }
    }

    Ok(None)
}

fn parse_repl_invocation<I>(args: &mut std::iter::Peekable<I>) -> CliResult<ReplOptions>
where
    I: Iterator<Item = String>,
{
    let mut options = CompilationOptions::default();
    let mut preload = Vec::new();
    let mut autorun = false;
    let mut show_pipeline_summary = false;
    let mut verbose = false;
    let mut json_output = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" => {
                for remaining in args {
                    preload.push(PathBuf::from(remaining));
                }
                break;
            }
            "--dump-ast" => options.dump_ast = true,
            "--dump-ir" => options.dump_ir = true,
            "--timings" | "-T" => {
                options.collect_metrics = true;
                show_pipeline_summary = true;
            }
            "--no-optimize" | "-O0" => {
                options.optimize = false;
                options.opt_level = 0;
            }
            "-O1" => {
                options.optimize = true;
                options.opt_level = 1;
            }
            "-O2" => {
                options.optimize = true;
                options.opt_level = 2;
            }
            "-O3" => {
                options.optimize = true;
                options.opt_level = 3;
            }
            "--run" | "-r" => {
                autorun = true;
                options.run_jit = true;
            }
            "--summary" | "--pipeline-summary" => {
                options.collect_metrics = true;
                show_pipeline_summary = true;
            }
            "--json" => {
                json_output = true;
            }
            "--verbose" | "-v" => verbose = true,
            "--enable-experimental" => {
                if let Some(feature) = args.next() {
                    options.experimental_features.insert(feature);
                } else {
                    return Err(usage_error(
                        "Missing feature name after '--enable-experimental'.",
                    ));
                }
            }
            "--list-experimental" => {
                return Err(usage_error(
                    "--list-experimental must appear before any command.",
                ));
            }
            flag if flag.starts_with('-') => {
                return Err(usage_error(&format!("Unknown option: {}", flag)));
            }
            _ => preload.push(PathBuf::from(arg)),
        }
    }

    Ok(ReplOptions {
        base_options: options,
        preload,
        autorun,
        show_pipeline_summary,
        verbose,
        json_output,
    })
}

fn parse_new_project_invocation<I>(
    args: &mut std::iter::Peekable<I>,
) -> CliResult<NewProjectOptions>
where
    I: Iterator<Item = String>,
{
    let mut path: Option<PathBuf> = None;
    let mut force = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--force" | "-f" => force = true,
            flag if flag.starts_with('-') => {
                return Err(usage_error(&format!("Unknown option: {}", flag)));
            }
            value => {
                if path.is_some() {
                    return Err(usage_error(
                        "Multiple locations provided. Supply exactly one project path.",
                    ));
                }
                path = Some(PathBuf::from(value));
            }
        }
    }

    let path = path.ok_or_else(|| usage_error("No project path supplied."))?;

    Ok(NewProjectOptions { path, force })
}

fn parse_format_invocation<I>(args: &mut std::iter::Peekable<I>) -> CliResult<FormatOptions>
where
    I: Iterator<Item = String>,
{
    let mut entries = Vec::new();
    let mut check = false;
    let mut use_stdin = false;
    let mut write_stdout = false;
    let mut stats = false;
    let mut explain = ExplainMode::None;
    let mut config_path: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" => {
                for value in args {
                    entries.push(PathBuf::from(value));
                }
                break;
            }
            "--check" => check = true,
            "--stdin" => use_stdin = true,
            "--stdout" => write_stdout = true,
            "--explain" => {
                if explain != ExplainMode::None {
                    return Err(usage_error(
                        "Multiple --explain options provided. Specify it at most once.",
                    ));
                }
                explain = ExplainMode::Text;
                check = true;
            }
            flag if flag.starts_with("--explain=") => {
                if explain != ExplainMode::None {
                    return Err(usage_error(
                        "Multiple --explain options provided. Specify it at most once.",
                    ));
                }
                let value = &flag[10..];
                explain = match value {
                    "text" => ExplainMode::Text,
                    "json" => ExplainMode::Json,
                    other => {
                        return Err(usage_error(&format!(
                            "Unknown --explain mode '{}'. Use 'text' or 'json'.",
                            other
                        )))
                    }
                };
                check = true;
            }
            "--config" => {
                if config_path.is_some() {
                    return Err(usage_error(
                        "Multiple --config options provided. Supply at most one configuration path.",
                    ));
                }
                if let Some(value) = args.next() {
                    config_path = Some(PathBuf::from(value));
                } else {
                    return Err(usage_error("Missing path argument after '--config'."));
                }
            }
            "--stats" => {
                stats = true;
            }
            flag if flag.starts_with('-') => {
                return Err(usage_error(&format!("Unknown option: {}", flag)));
            }
            _ => entries.push(PathBuf::from(arg)),
        }
    }

    if use_stdin && !entries.is_empty() {
        return Err(usage_error(
            "--stdin cannot be combined with explicit file or directory paths.",
        ));
    }

    if !use_stdin && entries.is_empty() {
        return Err(usage_error(
            "No source files or directories were provided for formatting.",
        ));
    }

    Ok(FormatOptions {
        entries,
        check,
        use_stdin,
        write_stdout,
        explain,
        stats,
        config_path,
    })
}

fn execute_build_command(kind: BuildCommand, invocation: CliInvocation) -> CliResult<()> {
    let CliInvocation {
        entries,
        options,
        show_pipeline_summary,
        verbose,
        json_output,
        emit_object,
        emit_exe,
        program_args,
    } = invocation;

    if kind == BuildCommand::Lint && json_output {
        return execute_lint_json(entries, options);
    }

    // AOT object emission: compile the first entry file and write the object bytes.
    if let Some(ref obj_path) = emit_object {
        if entries.is_empty() {
            return Err(CliError::usage("--emit-object requires a source file."));
        }
        let source_path = &entries[0];
        let source = fs::read_to_string(source_path)
            .map_err(|e| CliError::io(format!("Cannot read '{}': {}", source_path.display(), e)))?;
        let filename = source_path.to_string_lossy().to_string();
        let mut compiler = SpectraCompiler::new(options);
        compiler.set_emit_output(false);
        let obj_bytes = compiler
            .compile_to_object_bytes(&source, &filename)
            .map_err(|e| CliError::compilation(e))?;
        fs::write(obj_path, &obj_bytes)
            .map_err(|e| CliError::io(format!("Cannot write '{}': {}", obj_path.display(), e)))?;
        println!("✅ Object file written: {}", obj_path.display());
        println!(
            "   Link with: cc {} -L<runtime_lib_dir> -lspectra_runtime -o <output>",
            obj_path.display()
        );
        return Ok(());
    }

    // Executable compilation: compile → exe-object (with main shim) → link.
    if let Some(ref exe_path) = emit_exe {
        if entries.is_empty() {
            return Err(CliError::usage("--emit-exe requires a source file."));
        }
        let source_path = &entries[0];
        let source = fs::read_to_string(source_path)
            .map_err(|e| CliError::io(format!("Cannot read '{}': {}", source_path.display(), e)))?;
        let filename = source_path.to_string_lossy().to_string();

        // Locate the runtime static library before spending time compiling.
        let runtime_lib = runtime_lib::find_runtime_lib().ok_or_else(|| {
            CliError::compilation(
                "Cannot find libspectra_runtime.a / spectra_runtime.lib.\n\
                 Build the workspace first (`cargo build`) or set the \
                 SPECTRA_RUNTIME_LIB environment variable.",
            )
        })?;

        // Write the executable object to a temporary path next to the output.
        let obj_path = exe_path.with_extension("spectra_tmp.obj");

        let mut compiler = SpectraCompiler::new(options);
        compiler.set_emit_output(false);
        let obj_bytes = compiler
            .compile_to_executable_object_bytes(&source, &filename)
            .map_err(|e| CliError::compilation(e))?;

        fs::write(&obj_path, &obj_bytes).map_err(|e| {
            CliError::io(format!(
                "Cannot write temporary object '{}': {}",
                obj_path.display(),
                e
            ))
        })?;

        let link_result = linker::link_executable(&obj_path, &runtime_lib, exe_path);
        let _ = fs::remove_file(&obj_path); // always clean up the temp object
        link_result.map_err(|e| CliError::compilation(e))?;

        println!("✅ Executable written: {}", exe_path.display());
        return Ok(());
    }

    // For `run`: forward program arguments to the runtime before executing.
    // argv[0] is conventionally the script/exe path; additional args follow.
    if kind == BuildCommand::Run {
        let script_path = entries
            .first()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut effective_args = vec![script_path];
        effective_args.extend(program_args);
        forward_program_args(effective_args);
    }

    // If a single directory is given (or current dir when no entries), look for spectra.toml.
    let project_root: Option<std::path::PathBuf> = if entries.len() <= 1 {
        let candidate = entries
            .first()
            .map(|p| {
                if p.is_dir() {
                    p.clone()
                } else {
                    p.parent()
                        .map(|par| par.to_path_buf())
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                }
            })
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        Some(candidate)
    } else {
        None
    };

    let (final_entries, package_name) = if let Some(ref root) = project_root {
        match config::try_load_config(root) {
            Ok(Some(cfg)) => {
                if verbose {
                    println!(
                        "Loaded project config '{}' v{}",
                        cfg.name(),
                        cfg.project.version
                    );
                }
                let src_dirs = cfg.src_dirs(root);
                let sources = discovery::discover_sources(&src_dirs);
                (sources, Some(cfg.name().to_string()))
            }
            Ok(None) => (entries, None),
            Err(err) => {
                return Err(CliError::io(format!(
                    "Failed to load spectra.toml: {}",
                    err
                )));
            }
        }
    } else {
        (entries, None)
    };

    execute_plan_with_options(
        kind,
        options,
        final_entries,
        package_name,
        show_pipeline_summary,
        show_pipeline_summary,
        true,
        verbose,
    )
}

fn compile_plan(
    kind: BuildCommand,
    compiler: &mut SpectraCompiler,
    plan: &ProjectPlan,
    show_pipeline_summary: bool,
    verbose: bool,
) -> bool {
    // When running via JIT without verbose/timings, suppress build progress output
    // so only the Spectra program's own stdout/stderr reaches the terminal.
    let quiet = kind == BuildCommand::Run && !verbose;
    let mut has_failures = false;

    for module in plan.modules() {
        if !quiet {
            println!(
                "\n{} module: {} ({})",
                kind.module_verb(),
                module.name,
                module.path.display()
            );
        }

        if verbose {
            if module.imports.is_empty() {
                println!("    imports: (none)");
            } else {
                println!("    imports: {}", module.imports.join(", "));
            }
        }

        let filename = module.path.to_string_lossy().to_string();
        match fs::read_to_string(&module.path) {
            Ok(source) => {
                // When the source file has no explicit `module` declaration the
                // project plan already derived a name from the filename stem.
                // Prepend a synthetic header so the parser receives a valid AST
                // without requiring boilerplate in every script.
                let owned;
                let effective_source: &str = if source_has_module_decl(&source) {
                    &source
                } else {
                    owned = format!("module {};\n{}", module.name, source);
                    &owned
                };

                match compiler.compile(effective_source, &filename) {
                    Ok(()) => {
                        if !quiet {
                            println!(
                                "\nSuccessfully {} module '{}'",
                                kind.module_success_verb(),
                                module.name
                            );
                        }
                        if show_pipeline_summary {
                            if let Some(summary) = compiler.take_last_summary() {
                                print_pipeline_summary(&summary);
                            }
                        }
                    }
                    Err(error) => {
                        has_failures = true;
                        // Print the pre-formatted diagnostic block directly to stderr.
                        // `render_errors()` already produces a fully structured
                        // "error[phase]: msg\n  --> file:line:col\n ..." block,
                        // including aligned source spans and carets.  Passing it
                        // through `log_error()` would add a spurious "error: "
                        // prefix to the first line and 7-space indent to every
                        // subsequent line, breaking gutter alignment.
                        eprint!("{}", error);
                    }
                }
            }
            Err(error) => {
                has_failures = true;
                eprintln!(
                    "error[io]: cannot read '{}': {}",
                    module.path.display(),
                    error
                );
            }
        }
    }

    has_failures
}

/// Returns `true` when the source already contains an explicit `module <name>;`
/// declaration at the start of the file, ignoring blank lines and both `//`
/// line comments and `/* */` block comments.
fn source_has_module_decl(source: &str) -> bool {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    loop {
        // Skip whitespace
        while i < len && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }

        if i >= len {
            return false;
        }

        if bytes[i] == b'/' {
            if i + 1 < len && bytes[i + 1] == b'/' {
                // Skip line comment
                i += 2;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            } else if i + 1 < len && bytes[i + 1] == b'*' {
                // Skip block comment
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2; // consume '*/'
                continue;
            }
        }

        // Next non-whitespace, non-comment content: check for `module `
        return bytes[i..].starts_with(b"module ");
    }
}

fn print_pipeline_summary(summary: &ModulePipelineSummary) {
    println!("    Pipeline summary:");
    println!("      Source: {}", summary.filename);

    if let Some(metrics) = &summary.frontend_metrics {
        println!("      Front-end total: {:?}", metrics.total);
        println!("        • Lexing:    {:?}", metrics.lexing);
        println!("        • Parsing:   {:?}", metrics.parsing);
        println!("        • Semantic:  {:?}", metrics.semantic);
        println!("        • Backend:   {:?}", metrics.backend);
    } else {
        println!("      Front-end timings unavailable (enable --timings to collect).",);
    }

    println!("      Lowering: {:?}", summary.lowering_duration);
    println!("      Codegen:  {:?}", summary.codegen_duration);

    if !summary.passes.is_empty() {
        println!("      Passes:");
        for pass in &summary.passes {
            let status = if pass.modified {
                "modified"
            } else {
                "no change"
            };
            println!(
                "        - {:<24} {:>10?} ({})",
                pass.name, pass.duration, status
            );
        }
    }
}

fn execute_plan_with_options(
    kind: BuildCommand,
    options: CompilationOptions,
    entries: Vec<PathBuf>,
    package_name: Option<String>,
    show_pipeline_summary: bool,
    show_aggregate_summary: bool,
    print_success: bool,
    verbose: bool,
) -> CliResult<()> {
    let plan = ProjectPlan::build(entries).map_err(|error| CliError::io(error.to_string()))?;

    if plan.modules().is_empty() {
        return Err(CliError::usage("No Spectra source files found to compile."));
    }

    if verbose {
        print_verbose_configuration(kind, &options);
        println!();
        println!(
            "Project plan contains {} module{}:",
            plan.modules().len(),
            if plan.modules().len() == 1 { "" } else { "s" }
        );
        for (index, module) in plan.modules().iter().enumerate() {
            println!(
                "  {:>2}. {} ({})",
                index + 1,
                module.name,
                module.path.display()
            );
            if !module.imports.is_empty() {
                println!("       imports: {}", module.imports.join(", "));
            }
        }
    }

    let mut compiler = SpectraCompiler::new(options);
    if let Some(name) = package_name {
        compiler.set_package_name(name);
    }

    if show_pipeline_summary {
        compiler.set_emit_internal_metrics(false);
    }

    // For `run` without `--verbose` / `--timings`, suppress compile banners and
    // the post-execution metadata line so only the program's output is visible.
    if kind == BuildCommand::Run && !verbose {
        compiler.set_emit_output(false);
        compiler.set_quiet_execution(true);
    }

    let has_failures = compile_plan(kind, &mut compiler, &plan, show_pipeline_summary, verbose);

    if show_aggregate_summary {
        compiler.print_aggregate_summary();
    }

    if has_failures {
        return Err(CliError::compilation(
            "could not compile due to previous error(s)",
        ));
    }

    // Propagate the Spectra program's exit code when running via JIT.
    if kind == BuildCommand::Run {
        if let Some(code) = take_last_exec_exit() {
            if code != 0 {
                std::process::exit(code);
            }
        }
    }

    if print_success && kind != BuildCommand::Run {
        println!("\n{}", kind.success_message());
    }

    Ok(())
}

fn print_verbose_configuration(kind: BuildCommand, options: &CompilationOptions) {
    println!("Verbose mode enabled");
    println!("  • Command: {}", kind.name());
    println!(
        "  • Optimization level: O{} ({})",
        options.opt_level,
        if options.optimize {
            "optimizations on"
        } else {
            "optimizations off"
        }
    );
    println!(
        "  • Dump AST: {}",
        if options.dump_ast { "yes" } else { "no" }
    );
    println!(
        "  • Dump IR: {}",
        if options.dump_ir { "yes" } else { "no" }
    );
    println!(
        "  • Collect metrics: {}",
        if options.collect_metrics { "yes" } else { "no" }
    );
    println!(
        "  • Run JIT after build: {}",
        if options.run_jit { "yes" } else { "no" }
    );

    if options.lint.enabled.is_empty() {
        println!("  • Linting: disabled");
    } else {
        let mut denied: Vec<_> = options.lint.deny.iter().map(|rule| rule.code()).collect();
        denied.sort();
        let denied_display = if denied.is_empty() {
            "none".to_string()
        } else {
            denied.join(", ")
        };
        println!("  • Linting: enabled (denied rules: {})", denied_display);
    }

    let mut features: Vec<_> = options.experimental_features.iter().collect();
    features.sort();
    if features.is_empty() {
        println!("  • Experimental features: (none)");
    } else {
        println!(
            "  • Experimental features: {}",
            features
                .into_iter()
                .map(|feature| feature.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn execute_repl(options: ReplOptions) -> CliResult<()> {
    let ReplOptions {
        base_options,
        preload,
        autorun,
        show_pipeline_summary,
        verbose,
        json_output,
    } = options;

    if json_output {
        return execute_repl_json(base_options, preload);
    }

    let session = ReplSession::new(base_options, autorun, show_pipeline_summary, verbose);

    if !preload.is_empty() {
        if let Err(error) = session.compile_entries(preload, session.default_command(), true) {
            log_error(&error.message);
        }
    }

    session.run()
}

struct ReplSession {
    base_options: CompilationOptions,
    autorun: bool,
    show_pipeline_summary: bool,
    verbose: bool,
}

impl ReplSession {
    fn new(
        base_options: CompilationOptions,
        autorun: bool,
        show_pipeline_summary: bool,
        verbose: bool,
    ) -> Self {
        Self {
            base_options,
            autorun,
            show_pipeline_summary,
            verbose,
        }
    }

    fn default_command(&self) -> BuildCommand {
        if self.autorun {
            BuildCommand::Run
        } else {
            BuildCommand::Compile
        }
    }

    fn compile_entries(
        &self,
        entries: Vec<PathBuf>,
        command: BuildCommand,
        print_success: bool,
    ) -> CliResult<()> {
        if entries.is_empty() {
            return Err(CliError::usage("Provide one or more paths to compile."));
        }

        let mut options = self.base_options.clone();
        match command {
            BuildCommand::Run => options.run_jit = true,
            BuildCommand::Check | BuildCommand::Lint | BuildCommand::Compile => {
                options.run_jit = false
            }
        }

        execute_plan_with_options(
            command,
            options,
            entries,
            None,
            self.show_pipeline_summary,
            false,
            print_success,
            self.verbose,
        )
    }

    fn run(&self) -> CliResult<()> {
        println!("SpectraLang REPL (type ':help' for commands)");

        let stdin = io::stdin();

        loop {
            print!("spectra> ");
            io::stdout()
                .flush()
                .map_err(|error| CliError::io(format!("Failed to flush prompt: {}", error)))?;

            let mut line = String::new();
            let bytes = stdin
                .read_line(&mut line)
                .map_err(|error| CliError::io(format!("Failed to read input: {}", error)))?;

            if bytes == 0 {
                println!();
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with(':') {
                if !self.handle_command(trimmed)? {
                    break;
                }
                continue;
            }

            let entries: Vec<PathBuf> = trimmed.split_whitespace().map(PathBuf::from).collect();

            if let Err(error) = self.compile_entries(entries, self.default_command(), true) {
                log_error(&error.message);
            }
        }

        Ok(())
    }

    fn handle_command(&self, input: &str) -> CliResult<bool> {
        let command = input[1..].trim();
        if command.is_empty() {
            print_repl_help();
            return Ok(true);
        }

        let mut parts = command.split_whitespace();
        let keyword = parts.next().unwrap();
        let args: Vec<PathBuf> = parts.map(PathBuf::from).collect();

        match keyword {
            "help" | "h" => {
                print_repl_help();
                Ok(true)
            }
            "quit" | "q" | "exit" => Ok(false),
            "load" | "l" => {
                if args.is_empty() {
                    println!("Usage: :load <paths>...");
                    return Ok(true);
                }
                if let Err(error) = self.compile_entries(args, BuildCommand::Compile, true) {
                    log_error(&error.message);
                }
                Ok(true)
            }
            "run" => {
                if args.is_empty() {
                    println!("Usage: :run <paths>...");
                    return Ok(true);
                }
                if let Err(error) = self.compile_entries(args, BuildCommand::Run, true) {
                    log_error(&error.message);
                }
                Ok(true)
            }
            "check" => {
                if args.is_empty() {
                    println!("Usage: :check <paths>...");
                    return Ok(true);
                }
                if let Err(error) = self.compile_entries(args, BuildCommand::Check, true) {
                    log_error(&error.message);
                }
                Ok(true)
            }
            "compile" | "build" => {
                if args.is_empty() {
                    println!("Usage: :compile <paths>...");
                    return Ok(true);
                }
                if let Err(error) = self.compile_entries(args, BuildCommand::Compile, true) {
                    log_error(&error.message);
                }
                Ok(true)
            }
            unknown => {
                println!(
                    "Unknown REPL command ':{}'. Type ':help' for assistance.",
                    unknown
                );
                Ok(true)
            }
        }
    }
}

fn execute_new_project(options: NewProjectOptions) -> CliResult<()> {
    create_new_project(options)
}

fn create_new_project(options: NewProjectOptions) -> CliResult<()> {
    let NewProjectOptions { path, force } = options;

    if path.exists() {
        if !path.is_dir() {
            return Err(CliError::io(format!(
                "Path '{}' exists and is not a directory.",
                path.display()
            )));
        }

        if !force
            && !is_directory_empty(&path).map_err(|error| {
                CliError::io(format!("Failed to inspect '{}': {}", path.display(), error))
            })?
        {
            return Err(CliError::usage(format!(
                "Directory '{}' already exists. Use '--force' to scaffold anyway.",
                path.display()
            )));
        }
    }

    fs::create_dir_all(path.join("src")).map_err(|error| {
        CliError::io(format!(
            "Failed to create project directories under '{}': {}",
            path.display(),
            error
        ))
    })?;

    let (project_name, module_name) = derive_project_identifiers(&path);
    let manifest_path = path.join("spectra.toml");
    let main_source_path = path.join("src").join("main.spectra");

    let manifest_contents = format!(
        "[project]\nname = \"{}\"\nversion = \"0.1.0\"\nentry = \"src/main.spectra\"\nsrc_dirs = [\"src\"]\n\n[dependencies]\n# Add your dependencies here\n",
        project_name
    );

    let main_source = format!(
        "// SpectraLang starter module\n// Generated by `spectra new`\n\nmodule {};\n\nfn add(lhs: int, rhs: int) -> int {{\n    return lhs + rhs;\n}}\n\npub fn main() -> int {{\n    let first = 21;\n    let second = 21;\n    let total = add(first, second);\n    return total;\n}}\n",
        module_name
    );

    fs::write(&manifest_path, manifest_contents).map_err(|error| {
        CliError::io(format!(
            "Failed to write manifest '{}': {}",
            manifest_path.display(),
            error
        ))
    })?;

    fs::write(&main_source_path, main_source).map_err(|error| {
        CliError::io(format!(
            "Failed to write source file '{}': {}",
            main_source_path.display(),
            error
        ))
    })?;

    println!("✨ Created Spectra project at '{}'", path.display());
    println!("   • Manifest: {}", manifest_path.display());
    println!("   • Entry:    {}", main_source_path.display());
    println!();
    println!("Next steps:");
    println!("  1. spectra run {}", main_source_path.display());
    println!("  2. Explore and adjust 'Spectra.toml' to suit your project.");

    Ok(())
}

fn execute_format(options: FormatOptions) -> CliResult<()> {
    run_formatter(options)
}

fn execute_lint_json(entries: Vec<PathBuf>, mut options: CompilationOptions) -> CliResult<()> {
    if entries.is_empty() {
        return Err(CliError::usage(
            "No Spectra source files were provided for linting.",
        ));
    }

    options.run_jit = false;
    run_json_diagnostics(entries, options)
}

fn execute_repl_json(mut options: CompilationOptions, preload: Vec<PathBuf>) -> CliResult<()> {
    if preload.is_empty() {
        return Err(CliError::usage(
            "Provide one or more paths when using 'spectra repl --json'.",
        ));
    }

    configure_lint_options(&mut options, &preload, true, &[], &[])?;
    options.run_jit = false;
    run_json_diagnostics(preload, options)
}

fn run_json_diagnostics(entries: Vec<PathBuf>, options: CompilationOptions) -> CliResult<()> {
    let plan = match ProjectPlan::build(entries.clone()) {
        Ok(plan) => plan,
        Err(error) => {
            let path = entries
                .get(0)
                .cloned()
                .unwrap_or_else(|| PathBuf::from("."));

            let report = JsonDiagnosticReport {
                version: 1,
                success: false,
                files: vec![JsonFileDiagnostics {
                    path: path_to_string(&path),
                    diagnostics: vec![generic_error_diagnostic(format!("{}", error), Some("cli"))],
                }],
            };

            emit_json_report(&report, true)?;
            return Ok(());
        }
    };

    if plan.modules().is_empty() {
        let report = JsonDiagnosticReport {
            version: 1,
            success: true,
            files: Vec::new(),
        };
        emit_json_report(&report, false)?;
        return Ok(());
    }

    let mut compiler = SpectraCompiler::new(options);
    compiler.set_emit_internal_metrics(false);
    compiler.set_emit_output(false);

    let mut files: BTreeMap<PathBuf, Vec<JsonDiagnostic>> = BTreeMap::new();
    let mut has_errors = false;

    for module in plan.modules() {
        let path = module.path.clone();
        let display_path = path_to_string(&path);
        let diagnostics = files.entry(path.clone()).or_default();

        let source = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                diagnostics.push(io_error_diagnostic(&error));
                has_errors = true;
                continue;
            }
        };

        match compiler.compile_for_diagnostics(&source, &display_path) {
            Ok(warnings) => {
                for warning in warnings {
                    diagnostics.push(convert_lint_diagnostic(warning));
                }
            }
            Err(errors) => {
                has_errors = true;
                for error in errors {
                    diagnostics.push(convert_compiler_error(error));
                }
            }
        }
    }

    let files: Vec<JsonFileDiagnostics> = files
        .into_iter()
        .map(|(path, diagnostics)| JsonFileDiagnostics {
            path: path_to_string(&path),
            diagnostics,
        })
        .collect();

    let report = JsonDiagnosticReport {
        version: 1,
        success: !has_errors,
        files,
    };

    emit_json_report(&report, has_errors)
}

fn emit_json_report(report: &JsonDiagnosticReport, has_errors: bool) -> CliResult<()> {
    let mut stdout = io::stdout();
    serde_json::to_writer(&mut stdout, report).map_err(|error| {
        CliError::io(format!(
            "Failed to serialize diagnostics to JSON: {}",
            error
        ))
    })?;
    stdout
        .write_all(b"\n")
        .map_err(|error| CliError::io(format!("Failed to write diagnostics: {}", error)))?;
    stdout
        .flush()
        .map_err(|error| CliError::io(format!("Failed to flush diagnostics: {}", error)))?;

    if has_errors {
        process::exit(ExitCode::CompilationFailed.as_i32());
    }

    Ok(())
}

fn convert_lint_diagnostic(diagnostic: LintDiagnostic) -> JsonDiagnostic {
    let LintDiagnostic {
        rule,
        message,
        span,
        note,
        secondary_span,
    } = diagnostic;

    let mut related = Vec::new();
    if let Some(secondary) = secondary_span {
        related.push(JsonRelated {
            message: "related location".to_string(),
            range: Some(span_to_range(&secondary)),
        });
    }

    JsonDiagnostic {
        severity: JsonSeverity::Warning,
        code: Some(format!("lint({})", rule.code())),
        message,
        phase: Some("lint".to_string()),
        hint: note,
        range: span_to_range(&span),
        related,
    }
}

fn convert_compiler_error(error: CompilerError) -> JsonDiagnostic {
    match error {
        CompilerError::Lexical(e) => {
            span_error_to_json("lexical", e.message, e.span, e.context, e.hint)
        }
        CompilerError::Parse(e) => {
            span_error_to_json("parse", e.message, e.span, e.context, e.hint)
        }
        CompilerError::Semantic(e) => {
            span_error_to_json("semantic", e.message, e.span, e.context, e.hint)
        }
        CompilerError::Midend(e) => {
            generic_error_diagnostic(format!("midend error: {}", e.message), Some("midend"))
        }
        CompilerError::Backend(e) => {
            generic_error_diagnostic(format!("backend error: {}", e.message), Some("backend"))
        }
    }
}

fn span_error_to_json(
    phase: &'static str,
    message: String,
    span: Span,
    context: Option<String>,
    hint: Option<String>,
) -> JsonDiagnostic {
    let mut related = Vec::new();
    if let Some(context) = context {
        related.push(JsonRelated {
            message: context,
            range: None,
        });
    }

    JsonDiagnostic {
        severity: JsonSeverity::Error,
        code: Some(phase.to_string()),
        message,
        phase: Some(phase.to_string()),
        hint,
        range: span_to_range(&span),
        related,
    }
}

fn io_error_diagnostic(error: &io::Error) -> JsonDiagnostic {
    generic_error_diagnostic(format!("I/O error: {}", error), Some("io"))
}

fn generic_error_diagnostic(message: String, phase: Option<&str>) -> JsonDiagnostic {
    JsonDiagnostic {
        severity: JsonSeverity::Error,
        code: phase.map(|value| value.to_string()),
        message,
        phase: phase.map(|value| value.to_string()),
        hint: None,
        range: default_range(),
        related: Vec::new(),
    }
}

fn span_to_range(span: &Span) -> JsonRange {
    JsonRange {
        start: JsonPosition {
            line: span.start_location.line,
            column: span.start_location.column,
        },
        end: JsonPosition {
            line: span.end_location.line,
            column: span.end_location.column,
        },
    }
}

fn default_range() -> JsonRange {
    JsonRange {
        start: JsonPosition { line: 1, column: 1 },
        end: JsonPosition { line: 1, column: 1 },
    }
}

fn path_to_string(path: &Path) -> String {
    fs::canonicalize(path)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

#[derive(Serialize)]
struct JsonDiagnosticReport {
    version: u8,
    success: bool,
    files: Vec<JsonFileDiagnostics>,
}

#[derive(Serialize)]
struct JsonFileDiagnostics {
    path: String,
    diagnostics: Vec<JsonDiagnostic>,
}

#[derive(Serialize)]
struct JsonDiagnostic {
    severity: JsonSeverity,
    code: Option<String>,
    message: String,
    phase: Option<String>,
    hint: Option<String>,
    range: JsonRange,
    related: Vec<JsonRelated>,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum JsonSeverity {
    Error,
    Warning,
}

#[derive(Serialize)]
struct JsonRange {
    start: JsonPosition,
    end: JsonPosition,
}

#[derive(Serialize)]
struct JsonPosition {
    line: usize,
    column: usize,
}

#[derive(Serialize)]
struct JsonRelated {
    message: String,
    range: Option<JsonRange>,
}

fn derive_project_identifiers(path: &Path) -> (String, String) {
    let raw_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("spectra_app");

    let project_name = sanitize_project_name(raw_name);
    let module_name = sanitize_module_name(&project_name);

    (project_name, module_name)
}

fn sanitize_project_name(raw: &str) -> String {
    let mut result = String::new();

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '_' | '-' | ' ') {
            if !result.ends_with('_') && !result.is_empty() {
                result.push('_');
            }
        }
    }

    let trimmed = result.trim_matches('_');
    if trimmed.is_empty() {
        "spectra_app".to_string()
    } else {
        trimmed.to_string()
    }
}

fn sanitize_module_name(project_name: &str) -> String {
    let mut result = String::new();

    for ch in project_name.chars() {
        if result.is_empty() {
            if ch.is_ascii_alphabetic() {
                result.push(ch);
            } else if ch.is_ascii_digit() {
                result.push('m');
                result.push(ch);
            }
        } else if ch.is_ascii_alphanumeric() || ch == '_' {
            result.push(ch);
        }
    }

    if result.is_empty() {
        "app".to_string()
    } else {
        result
    }
}

fn is_directory_empty(path: &Path) -> Result<bool, io::Error> {
    let mut entries = fs::read_dir(path)?;
    Ok(entries.next().transpose()?.is_none())
}

fn print_global_help() {
    println!("SpectraLang CLI");
    println!();
    println!("USAGE:");
    println!("    spectralang <COMMAND> [OPTIONS] <paths>...");
    println!();
    println!("COMMANDS:");
    println!("    compile    Compile Spectra modules (default)");
    println!("    check      Type-check modules and report diagnostics");
    println!("    run        Compile modules and execute the entry point via JIT");
    println!("    lint       Run lint checks across Spectra modules");
    println!("    repl       Start an interactive Spectra prompt");
    println!("    new        Scaffold a new Spectra project");
    println!("    fmt        Format Spectra source files");
    println!("    help       Print this help message");
    println!();
    println!("GLOBAL OPTIONS:");
    println!("    -h, --help             Print this help message");
    println!("    --list-experimental    List available experimental features and exit");
    println!();
    print_compilation_options(None);
    println!();
    println!("EXAMPLES:");
    println!("    spectralang compile src/main.spectra");
    println!("    spectralang check examples/");
    println!("    spectralang run -O3 app.spectra");
    println!("    spectralang lint src/");
    println!("    spectralang repl --run");
    println!("    spectralang new my-project");
    println!("    spectralang --list-experimental");
    println!("    spectralang fmt src/");
    println!("    spectralang fmt --stdin < file.spectra");
    println!();
    print_experimental_features();
    println!();
    println!("EXIT CODES:");
    println!("    0   Success");
    println!("    64  Usage error (invalid flags, missing inputs)");
    println!("    65  Compilation failed");
    println!("    74  I/O failure while reading or writing files");
    println!();
    println!("LOGGING:");
    println!("    Errors are emitted as 'error: <message>' for easy parsing.");
}

fn print_build_help(command: BuildCommand) {
    println!("SpectraLang CLI – '{}' command", command.name());
    println!();
    println!("USAGE:");
    println!("    spectralang {} [OPTIONS] <paths>...", command.name());
    println!();
    println!("{}", command.description());
    println!();
    print_compilation_options(Some(command));
    println!();
    println!("Examples:");
    match command {
        BuildCommand::Compile => {
            println!("    spectralang compile src/main.spectra");
            println!("    spectralang compile --dump-ir project/");
        }
        BuildCommand::Check => {
            println!("    spectralang check src/");
            println!("    spectralang check --dump-ast main.spectra");
        }
        BuildCommand::Run => {
            println!("    spectralang run app.spectra");
            println!("    spectralang run --timings src/main.spectra");
        }
        BuildCommand::Lint => {
            println!("    spectralang lint src/");
            println!("    spectralang lint --deny shadowing examples/");
        }
    }
    println!();
    println!("Use 'spectralang --list-experimental' to see available experimental features.");
}

fn print_repl_help() {
    println!("SpectraLang CLI – 'repl' command");
    println!();
    println!("USAGE:");
    println!("    spectralang repl [OPTIONS] [paths]...");
    println!();
    println!("Starts an interactive prompt that can compile, check, or run Spectra modules.");
    println!();
    println!("OPTIONS:");
    println!("    --dump-ast             Print the AST for debugging when compiling");
    println!("    --dump-ir              Print the IR for debugging when compiling");
    println!("    --timings, -T          Report compilation and execution timings");
    println!("    --summary              Show pipeline summaries for compiled modules");
    println!("    --verbose, -v          Print additional build details");
    println!("    --no-optimize, -O0     Disable all optimizations");
    println!("    -O1/-O2/-O3            Set optimization level");
    println!("    --run, -r              Automatically run modules after compiling");
    println!("    --enable-experimental <feature>");
    println!("                           Enable experimental language feature (may be repeated)");
    println!();
    println!("Interactive commands:");
    println!("    :load <paths>...       Compile modules without executing");
    println!("    :run <paths>...        Compile and execute modules");
    println!("    :check <paths>...      Type-check modules only");
    println!("    :compile <paths>...    Alias for :load");
    println!("    :help                  Show this help text");
    println!("    :quit                  Exit the REPL");
}

fn print_new_help() {
    println!("SpectraLang CLI – 'new' command");
    println!();
    println!("USAGE:");
    println!("    spectralang new [OPTIONS] <path>");
    println!();
    println!("Create a new Spectra project with a starter module and manifest.");
    println!();
    println!("OPTIONS:");
    println!("    -f, --force        Scaffold even if the directory already exists");
    println!();
    println!("Examples:");
    println!("    spectralang new hello-world");
    println!("    spectralang new --force .");
}

fn print_format_help() {
    println!("SpectraLang CLI – 'fmt' command");
    println!();
    println!("USAGE:");
    println!("    spectralang fmt [OPTIONS] <paths>...");
    println!();
    println!("Format Spectra source files in-place or verify formatting with --check.");
    println!();
    println!("OPTIONS:");
    println!("    --check              Verify formatting without writing changes");
    println!("    --stdin              Read Spectra source from standard input");
    println!("    --stdout             Write the formatted result to stdout instead of files (single input file)");
    println!("    --explain[=json]     Show diffs (text by default, json for machine-readable) and implies --check");
    println!("    --stats              Emit a JSON summary of the formatter run");
    println!("    --config <path>      Load formatter configuration from an explicit Spectra.toml");
    println!("    -h, --help          Show this help text");
    println!();
    println!("Examples:");
    println!("    spectralang fmt src/");
    println!("    spectralang fmt --check examples/test.spectra");
    println!("    spectralang fmt --stdin < script.spectra");
    println!("    spectralang fmt --stdout src/main.spectra");
}

fn print_lint_help() {
    println!("SpectraLang CLI – 'lint' command");
    println!();
    println!("USAGE:");
    println!("    spectralang lint [OPTIONS] <paths>...");
    println!();
    println!("Run Spectra's lint checks across the provided files or directories.");
    println!("Warnings are reported to stdout; denied rules cause the command to fail with exit code 65.");
    println!();
    println!("OPTIONS:");
    println!("    --lint              Redundant; 'lint' always enables lint rules");
    println!("    --allow <rule>      Allow (suppress) a lint rule (may be repeated)");
    println!("    --deny <rule>       Deny a lint rule and escalate matches to errors");
    println!("    --dump-ast          Dump the parsed AST for debugging");
    println!("    --timings, -T       Collect front-end timings");
    println!("    --summary           Print pipeline summaries (semantic + lint)");
    println!("    --verbose, -v       Print additional plan diagnostics");
    println!("    --enable-experimental <feature>");
    println!("                        Enable experimental language feature (may repeat)");
    println!(
        "    -O0/-O1/-O2/-O3     Set optimization level (ignored by lint but accepted for parity)"
    );
    println!();
    println!("Available lint rules: {}", lint_rule_list());
    println!();
    println!("Examples:");
    println!("    spectralang lint src/");
    println!("    spectralang lint --deny shadowing examples/");
}

fn print_compilation_options(command: Option<BuildCommand>) {
    println!("COMPILATION OPTIONS:");
    println!("    --dump-ast             Print the AST for debugging");
    println!("    --dump-ir              Print the IR for debugging");
    println!("    --timings, -T          Report compilation and execution timings");
    println!("    --summary              Show pipeline summaries for compiled modules");
    println!("    --verbose, -v          Print additional build details");
    println!("    --no-optimize, -O0     Disable all optimizations");
    println!("    -O1                    Enable basic optimizations");
    println!("    -O2                    Enable moderate optimizations (default)");
    println!("    -O3                    Enable aggressive optimizations");
    match command {
        Some(BuildCommand::Check) | Some(BuildCommand::Lint) => {
            println!("    --run, -r              Not available for the 'check' command");
        }
        Some(BuildCommand::Run) => {
            println!("    --run, -r              Redundant; 'run' always executes after compiling");
        }
        _ => {
            println!("    --run, -r              Execute the program with the JIT after compiling");
        }
    }
    println!("    --enable-experimental <feature>");
    println!("                           Enable experimental language feature (may be repeated)");
    if matches!(command, Some(BuildCommand::Lint)) {
        println!("    --lint                 Redundant; 'lint' always enables lint rules");
    } else {
        println!("    --lint                 Enable lint checks for the selected command");
    }
    println!("    --allow <rule>         Allow (suppress) a lint rule (may be repeated)");
    println!("    --deny <rule>          Deny a lint rule and escalate matches to errors");
    println!(
        "                           Available rules: {}",
        lint_rule_list()
    );
}

fn print_experimental_features() {
    println!("Experimental features you can enable with --enable-experimental <feature>:");
    for feature in KNOWN_EXPERIMENTAL_FEATURES {
        println!("    - {}", feature);
    }
}

fn usage_error(message: &str) -> CliError {
    let trimmed = message.trim_end();
    let formatted = format!(
        "{}\nUse 'spectralang --help' for usage information.",
        trimmed
    );
    CliError::usage(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_values_are_stable() {
        assert_eq!(ExitCode::Success.as_i32(), 0);
        assert_eq!(ExitCode::Usage.as_i32(), 64);
        assert_eq!(ExitCode::CompilationFailed.as_i32(), 65);
        assert_eq!(ExitCode::IoError.as_i32(), 74);
    }

    #[test]
    fn usage_error_includes_help_hint() {
        let error = usage_error("Missing source");
        assert_eq!(error.code.as_i32(), ExitCode::Usage.as_i32());
        assert!(error.message.contains("Missing source"));
        assert!(error
            .message
            .contains("Use 'spectra --help' for usage information."));
    }

    #[test]
    fn cli_error_builders_assign_codes() {
        let compilation = CliError::compilation("failed");
        assert_eq!(
            compilation.code.as_i32(),
            ExitCode::CompilationFailed.as_i32()
        );

        let io = CliError::io("io issue");
        assert_eq!(io.code.as_i32(), ExitCode::IoError.as_i32());
    }
}
