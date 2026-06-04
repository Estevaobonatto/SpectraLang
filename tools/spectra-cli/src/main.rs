mod compiler_integration;
mod config;
mod discovery;
mod formatter;
mod linker;
mod package;
mod project;
mod runtime_lib;

use compiler_integration::{
    forward_program_args, take_last_exec_exit, ModulePipelineSummary, SpectraCompiler,
};
use formatter::{run as run_formatter, ExplainMode, FormatOptions};
use package::{PackageCommand, PackageInvocation};
use project::ProjectPlan;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
const AOT_DEBUG_MAP_SCHEMA_VERSION: u32 = 1;

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
    sarif_output: bool,
    /// When `Some(path)`, emit a native object file at `path` instead of / in addition to JIT.
    emit_object: Option<PathBuf>,
    /// When `Some(path)`, compile to a native executable at `path`.
    emit_exe: Option<PathBuf>,
    /// When `Some(path)`, write a machine-readable benchmark report.
    bench_json: Option<PathBuf>,
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
    Package(PackageInvocation),
    Format(FormatOptions),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum HelpTopic {
    Global,
    Build(BuildCommand),
    Repl,
    NewProject,
    Package,
    Format,
    Lint,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BuildCommand {
    Compile,
    Check,
    Run,
    Lint,
    Bench,
}

impl BuildCommand {
    fn name(self) -> &'static str {
        match self {
            BuildCommand::Compile => "compile",
            BuildCommand::Check => "check",
            BuildCommand::Run => "run",
            BuildCommand::Lint => "lint",
            BuildCommand::Bench => "bench",
        }
    }

    fn description(self) -> &'static str {
        match self {
            BuildCommand::Compile => "Compile Spectra modules (default).",
            BuildCommand::Check => "Type-check modules and report diagnostics without executing.",
            BuildCommand::Run => "Compile modules and execute the entry point via JIT.",
            BuildCommand::Lint => "Run lint checks and report warnings or denied rules.",
            BuildCommand::Bench => "Compile modules with timing metrics and optional JSON report.",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            BuildCommand::Compile => "    Finished",
            BuildCommand::Check => "    Finished (no errors detected)",
            BuildCommand::Run => "",
            BuildCommand::Lint => "    Finished (no lint findings)",
            BuildCommand::Bench => "    Finished bench",
        }
    }

    fn module_verb(self) -> &'static str {
        match self {
            BuildCommand::Check => "Checking",
            BuildCommand::Lint => "Linting",
            BuildCommand::Bench => "Benchmarking",
            BuildCommand::Compile | BuildCommand::Run => "Compiling",
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
                HelpTopic::Package => print_package_help(),
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
        CliAction::Package(invocation) => execute_package_command(invocation),
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
                    "package" | "pkg" => Ok(CliAction::Help(HelpTopic::Package)),
                    "repl" => Ok(CliAction::Help(HelpTopic::Repl)),
                    "fmt" | "format" => Ok(CliAction::Help(HelpTopic::Format)),
                    "lint" => Ok(CliAction::Help(HelpTopic::Lint)),
                    "bench" => Ok(CliAction::Help(HelpTopic::Build(BuildCommand::Bench))),
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
        Some("package") | Some("pkg") => {
            args.next();
            if let Some(flag) = args.peek() {
                if matches!(flag.as_str(), "--help" | "-h") {
                    args.next();
                    return Ok(CliAction::Help(HelpTopic::Package));
                }
            }

            let invocation = parse_package_invocation(&mut args)?;
            return Ok(CliAction::Package(invocation));
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
        "bench" => Some(BuildCommand::Bench),
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
    let mut sarif_output = false;
    let mut emit_object: Option<PathBuf> = None;
    let mut emit_exe: Option<PathBuf> = None;
    let mut bench_json: Option<PathBuf> = None;
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
                if matches!(command, BuildCommand::Run | BuildCommand::Bench) {
                    return Err(usage_error(
                        "'--json' is only supported with the 'compile', 'check', or 'lint' commands. Use '--bench-json <path>' for bench reports.",
                    ));
                }
                if sarif_output {
                    return Err(usage_error(
                        "'--json' and '--sarif' cannot be used together.",
                    ));
                }
                json_output = true;
            }
            "--sarif" => {
                if matches!(command, BuildCommand::Run | BuildCommand::Bench) {
                    return Err(usage_error(
                        "'--sarif' is only supported with the 'compile', 'check', or 'lint' commands. Use '--bench-json <path>' for bench reports.",
                    ));
                }
                if json_output {
                    return Err(usage_error(
                        "'--json' and '--sarif' cannot be used together.",
                    ));
                }
                sarif_output = true;
            }
            "--bench-json" => {
                let path = args
                    .next()
                    .ok_or_else(|| usage_error("Missing output path after '--bench-json'."))?;
                bench_json = Some(PathBuf::from(path));
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
        BuildCommand::Check | BuildCommand::Lint | BuildCommand::Bench => options.run_jit = false,
        BuildCommand::Compile => {}
    }
    if command == BuildCommand::Bench {
        options.collect_metrics = true;
        show_pipeline_summary = true;
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
        sarif_output,
        emit_object,
        emit_exe,
        bench_json,
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

fn parse_package_invocation<I>(args: &mut std::iter::Peekable<I>) -> CliResult<PackageInvocation>
where
    I: Iterator<Item = String>,
{
    let subcommand = args
        .next()
        .ok_or_else(|| usage_error("No package subcommand supplied."))?;
    let mut root = PathBuf::from(".");
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    let mut path: Option<PathBuf> = None;
    let mut registry: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage_error("Missing path after '--root'."))?;
                root = PathBuf::from(value);
            }
            "--version" => {
                version = Some(
                    args.next()
                        .ok_or_else(|| usage_error("Missing value after '--version'."))?,
                );
            }
            "--path" => {
                path =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        usage_error("Missing path after '--path'.")
                    })?));
            }
            "--registry" => {
                registry =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        usage_error("Missing path after '--registry'.")
                    })?));
            }
            flag if flag.starts_with('-') => {
                return Err(usage_error(&format!("Unknown package option: {}", flag)));
            }
            value => {
                if name.is_some() {
                    return Err(usage_error(
                        "Multiple package names supplied. Provide exactly one name.",
                    ));
                }
                name = Some(value.to_string());
            }
        }
    }

    let command = match subcommand.as_str() {
        "lock" => PackageCommand::Lock,
        "build" => PackageCommand::Build,
        "check" => PackageCommand::Check,
        "run" => PackageCommand::Run,
        "test" => PackageCommand::Test,
        "bench" => PackageCommand::Bench,
        "doc" => PackageCommand::Doc,
        "update" => PackageCommand::Update,
        "add" => PackageCommand::Add {
            name: name.ok_or_else(|| usage_error("package add requires a package name."))?,
            version,
            path,
            registry,
        },
        "publish" => PackageCommand::Publish {
            registry: registry
                .ok_or_else(|| usage_error("package publish requires --registry <path>."))?,
        },
        other => {
            return Err(usage_error(&format!(
                "Unknown package subcommand '{}'.",
                other
            )));
        }
    };

    Ok(PackageInvocation { root, command })
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
        sarif_output,
        emit_object,
        emit_exe,
        bench_json,
        program_args,
    } = invocation;

    if json_output || sarif_output {
        return match kind {
            BuildCommand::Compile | BuildCommand::Check | BuildCommand::Lint => {
                execute_structured_diagnostics(entries, options, sarif_output)
            }
            BuildCommand::Run | BuildCommand::Bench => Err(usage_error(
                "'--json' and '--sarif' are only supported with the 'compile', 'check', or 'lint' commands. Use '--bench-json <path>' for bench reports.",
            )),
        };
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
        let debug_map_path = write_aot_debug_map(
            source_path,
            obj_path,
            &source,
            AotArtifactKind::Object,
            &["gdb", "lldb"],
        )?;
        println!("     Written object {}", obj_path.display());
        println!("     Written debug map {}", debug_map_path.display());
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

        let debug_map_path = write_aot_debug_map(
            source_path,
            exe_path,
            &source,
            AotArtifactKind::Executable,
            &["gdb", "lldb", "cdb"],
        )?;
        println!("     Written executable {}", exe_path.display());
        println!("     Written debug map {}", debug_map_path.display());
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
                match package::resolve(root) {
                    Ok(workspace) if workspace.packages.len() > 1 => {
                        (workspace.source_entries(), workspace.root_package_name())
                    }
                    _ => {
                        let src_dirs = cfg.src_dirs(root);
                        let sources = discovery::discover_sources(&src_dirs);
                        (sources, Some(cfg.name().to_string()))
                    }
                }
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
        bench_json,
    )
}

fn compile_plan(
    kind: BuildCommand,
    compiler: &mut SpectraCompiler,
    plan: &ProjectPlan,
    show_pipeline_summary: bool,
    verbose: bool,
) -> (bool, Vec<ModulePipelineSummary>) {
    // When running via JIT without verbose/timings, suppress build progress output
    // so only the Spectra program's own stdout/stderr reaches the terminal.
    let quiet = kind == BuildCommand::Run && !verbose;
    let mut has_failures = false;
    let mut summaries = Vec::new();

    for module in plan.modules() {
        if !quiet {
            println!(
                "{:>12} {} ({})",
                kind.module_verb(),
                module.name,
                module.path.display()
            );
        }

        if verbose {
            if module.imports.is_empty() {
                println!("             imports: (none)");
            } else {
                println!("             imports: {}", module.imports.join(", "));
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
                        if let Some(summary) = compiler.take_last_summary() {
                            if show_pipeline_summary {
                                print_pipeline_summary(&summary);
                            }
                            summaries.push(summary);
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

    (has_failures, summaries)
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
        println!("        - Lexing:    {:?}", metrics.lexing);
        println!("        - Parsing:   {:?}", metrics.parsing);
        println!("        - Semantic:  {:?}", metrics.semantic);
        println!("        - Backend:   {:?}", metrics.backend);
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

fn write_bench_report(path: &Path, summaries: &[ModulePipelineSummary]) -> CliResult<()> {
    let report = BenchReport {
        version: 1,
        modules: summaries.iter().map(BenchModuleReport::from).collect(),
        totals: BenchTotals {
            modules: summaries.len(),
            frontend_ms: summaries
                .iter()
                .filter_map(|summary| summary.frontend_metrics.as_ref())
                .map(|metrics| duration_ms(metrics.total))
                .sum(),
            lowering_ms: summaries
                .iter()
                .map(|summary| duration_ms(summary.lowering_duration))
                .sum(),
            codegen_ms: summaries
                .iter()
                .map(|summary| duration_ms(summary.codegen_duration))
                .sum(),
            passes_ms: summaries
                .iter()
                .flat_map(|summary| summary.passes.iter())
                .map(|pass| duration_ms(pass.duration))
                .sum(),
        },
    };
    let text = serde_json::to_string_pretty(&report)
        .map_err(|error| CliError::io(format!("Failed to serialize bench report: {}", error)))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                CliError::io(format!(
                    "Failed to create bench report directory '{}': {}",
                    parent.display(),
                    error
                ))
            })?;
        }
    }
    fs::write(path, text).map_err(|error| {
        CliError::io(format!(
            "Failed to write bench report '{}': {}",
            path.display(),
            error
        ))
    })
}

#[derive(Serialize)]
struct BenchReport {
    version: u8,
    modules: Vec<BenchModuleReport>,
    totals: BenchTotals,
}

#[derive(Serialize)]
struct BenchModuleReport {
    file: String,
    frontend_ms: Option<f64>,
    lexing_ms: Option<f64>,
    parsing_ms: Option<f64>,
    semantic_ms: Option<f64>,
    backend_ms: Option<f64>,
    lowering_ms: f64,
    codegen_ms: f64,
    passes: Vec<BenchPassReport>,
}

impl From<&ModulePipelineSummary> for BenchModuleReport {
    fn from(summary: &ModulePipelineSummary) -> Self {
        let metrics = summary.frontend_metrics.as_ref();
        Self {
            file: summary.filename.clone(),
            frontend_ms: metrics.map(|metrics| duration_ms(metrics.total)),
            lexing_ms: metrics.map(|metrics| duration_ms(metrics.lexing)),
            parsing_ms: metrics.map(|metrics| duration_ms(metrics.parsing)),
            semantic_ms: metrics.map(|metrics| duration_ms(metrics.semantic)),
            backend_ms: metrics.map(|metrics| duration_ms(metrics.backend)),
            lowering_ms: duration_ms(summary.lowering_duration),
            codegen_ms: duration_ms(summary.codegen_duration),
            passes: summary
                .passes
                .iter()
                .map(|pass| BenchPassReport {
                    name: pass.name.to_string(),
                    duration_ms: duration_ms(pass.duration),
                    modified: pass.modified,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct BenchPassReport {
    name: String,
    duration_ms: f64,
    modified: bool,
}

#[derive(Serialize)]
struct BenchTotals {
    modules: usize,
    frontend_ms: f64,
    lowering_ms: f64,
    codegen_ms: f64,
    passes_ms: f64,
}

fn duration_ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
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
    bench_json: Option<PathBuf>,
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

    let (has_failures, summaries) =
        compile_plan(kind, &mut compiler, &plan, show_pipeline_summary, verbose);

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
        match take_last_exec_exit() {
            Some(code) => {
                if code != 0 {
                    if let Some((path, line, column)) = find_main_location(&plan) {
                        eprintln!(
                            "error[runtime]: program exited with status {}\n  --> {}:{}:{}\n   |\n   = stack:\n     0: main() at {}:{}:{}\n   = help: inspect frame 0 and rerun with '--timings' for pipeline context",
                            code,
                            path.display(),
                            line,
                            column,
                            path.display(),
                            line,
                            column
                        );
                    } else {
                        eprintln!(
                            "error[runtime]: program exited with status {}\n   = stack:\n     0: <entry point unavailable>\n   = help: rerun with '--timings' for pipeline context",
                            code
                        );
                    }
                    std::process::exit(code);
                }
            }
            None => {
                // No module defined `main` — nothing was executed.
                return Err(CliError::compilation(
                    "no entry point 'main' found; define a 'pub fn main() -> int' function",
                ));
            }
        }
    }

    if print_success && kind != BuildCommand::Run {
        let msg = kind.success_message();
        if !msg.is_empty() {
            println!("{}", msg);
        }
    }

    if kind == BuildCommand::Bench {
        if let Some(path) = bench_json {
            write_bench_report(&path, &summaries)?;
            println!("     Written bench report {}", path.display());
        }
    }

    Ok(())
}

fn find_main_location(plan: &ProjectPlan) -> Option<(PathBuf, usize, usize)> {
    for module in plan.modules() {
        let source = fs::read_to_string(&module.path).ok()?;
        if let Some((line_index, column_index, _line_text)) = find_main_location_in_source(&source)
        {
            return Some((module.path.clone(), line_index + 1, column_index + 1));
        }
    }
    None
}

#[derive(Clone, Copy)]
enum AotArtifactKind {
    Object,
    Executable,
}

impl AotArtifactKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Executable => "executable",
        }
    }
}

#[derive(Serialize)]
struct AotDebugMap<'a> {
    schema: &'a str,
    schema_version: u32,
    artifact: AotDebugArtifact,
    source: AotDebugSource,
    entrypoint: Option<AotDebugEntrypoint>,
    native_debuggers: &'a [&'a str],
    strategy: &'a str,
}

#[derive(Serialize)]
struct AotDebugArtifact {
    kind: String,
    path: String,
}

#[derive(Serialize)]
struct AotDebugSource {
    path: String,
}

#[derive(Serialize)]
struct AotDebugEntrypoint {
    function: String,
    exported_symbol: String,
    source_line: usize,
    source_column: usize,
    source_text: String,
}

fn write_aot_debug_map(
    source_path: &Path,
    artifact_path: &Path,
    source: &str,
    artifact_kind: AotArtifactKind,
    native_debuggers: &'static [&'static str],
) -> CliResult<PathBuf> {
    let entrypoint =
        if let Some((line_index, column_index, line_text)) = find_main_location_in_source(source) {
            Some(AotDebugEntrypoint {
                function: "main".to_string(),
                exported_symbol: match artifact_kind {
                    AotArtifactKind::Object => "main",
                    AotArtifactKind::Executable => "spectra_user_main",
                }
                .to_string(),
                source_line: line_index + 1,
                source_column: column_index + 1,
                source_text: line_text.trim().to_string(),
            })
        } else {
            if matches!(artifact_kind, AotArtifactKind::Executable) {
                return Err(CliError::compilation(
                "cannot emit executable AOT debug map because no 'fn main' entry point was found",
            ));
            }
            None
        };

    let debug_map_path = debug_map_path_for_artifact(artifact_path);
    let map = AotDebugMap {
        schema: "spectra-aot-debug-map",
        schema_version: AOT_DEBUG_MAP_SCHEMA_VERSION,
        artifact: AotDebugArtifact {
            kind: artifact_kind.as_str().to_string(),
            path: display_path(artifact_path),
        },
        source: AotDebugSource {
            path: display_path(source_path),
        },
        entrypoint,
        native_debuggers,
        strategy: "Break on the exported symbol in the native debugger and use this map to resolve the Spectra source span until native DWARF/PDB emission is available.",
    };
    let text = serde_json::to_string_pretty(&map)
        .map_err(|e| CliError::io(format!("Cannot serialize AOT debug map: {}", e)))?;
    fs::write(&debug_map_path, format!("{}\n", text)).map_err(|e| {
        CliError::io(format!(
            "Cannot write AOT debug map '{}': {}",
            debug_map_path.display(),
            e
        ))
    })?;
    Ok(debug_map_path)
}

fn debug_map_path_for_artifact(artifact_path: &Path) -> PathBuf {
    let mut debug_map_path = artifact_path.as_os_str().to_os_string();
    debug_map_path.push(".spectra-debug.json");
    PathBuf::from(debug_map_path)
}

fn find_main_location_in_source(source: &str) -> Option<(usize, usize, &str)> {
    for (line_index, line) in source.lines().enumerate() {
        let Some(column_index) = line.find("fn main") else {
            continue;
        };
        return Some((line_index, column_index, line));
    }
    None
}

fn display_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn print_verbose_configuration(kind: BuildCommand, options: &CompilationOptions) {
    println!("  - Command: {}", kind.name());
    println!(
        "  - Optimization level: O{} ({})",
        options.opt_level,
        if options.optimize {
            "optimizations on"
        } else {
            "optimizations off"
        }
    );
    println!(
        "  - Dump AST: {}",
        if options.dump_ast { "yes" } else { "no" }
    );
    println!(
        "  - Dump IR: {}",
        if options.dump_ir { "yes" } else { "no" }
    );
    println!(
        "  - Collect metrics: {}",
        if options.collect_metrics { "yes" } else { "no" }
    );
    println!(
        "  - Run JIT after build: {}",
        if options.run_jit { "yes" } else { "no" }
    );

    if options.lint.enabled.is_empty() {
        println!("  - Linting: disabled");
    } else {
        let mut denied: Vec<_> = options.lint.deny.iter().map(|rule| rule.code()).collect();
        denied.sort();
        let denied_display = if denied.is_empty() {
            "none".to_string()
        } else {
            denied.join(", ")
        };
        println!("  - Linting: enabled (denied rules: {})", denied_display);
    }

    let mut features: Vec<_> = options.experimental_features.iter().collect();
    features.sort();
    if features.is_empty() {
        println!("  - Experimental features: (none)");
    } else {
        println!(
            "  - Experimental features: {}",
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
            BuildCommand::Check
            | BuildCommand::Lint
            | BuildCommand::Compile
            | BuildCommand::Bench => options.run_jit = false,
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
            None,
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

    println!("     Created \"{}\" project", path.display());
    println!("       entry: {}", main_source_path.display());
    println!(
        "         run: spectra run \"{}\"",
        main_source_path.display()
    );

    Ok(())
}

fn execute_format(options: FormatOptions) -> CliResult<()> {
    run_formatter(options)
}

fn execute_package_command(invocation: PackageInvocation) -> CliResult<()> {
    match invocation.command {
        PackageCommand::Lock | PackageCommand::Update => {
            let workspace = package::resolve(&invocation.root)
                .map_err(|error| CliError::io(error.to_string()))?;
            let path = package::write_lockfile(&workspace)
                .map_err(|error| CliError::io(error.to_string()))?;
            println!("     Locked {}", path.display());
            Ok(())
        }
        PackageCommand::Build | PackageCommand::Check | PackageCommand::Run => {
            let workspace = package::resolve(&invocation.root)
                .map_err(|error| CliError::io(error.to_string()))?;
            let lock_path = package::write_lockfile(&workspace)
                .map_err(|error| CliError::io(error.to_string()))?;
            let entries = workspace.source_entries();
            let kind = match invocation.command {
                PackageCommand::Build => BuildCommand::Compile,
                PackageCommand::Check => BuildCommand::Check,
                PackageCommand::Run => BuildCommand::Run,
                _ => unreachable!(),
            };
            println!("     Locked {}", lock_path.display());
            execute_plan_with_options(
                kind,
                CompilationOptions::default(),
                entries,
                workspace.root_package_name(),
                false,
                false,
                true,
                false,
                None,
            )
        }
        PackageCommand::Test => {
            let workspace = package::resolve(&invocation.root)
                .map_err(|error| CliError::io(error.to_string()))?;
            let mut entries = workspace.source_entries();
            entries.extend(package::discover_test_entries(&workspace));
            let lock_path = package::write_lockfile(&workspace)
                .map_err(|error| CliError::io(error.to_string()))?;
            println!("     Locked {}", lock_path.display());
            execute_plan_with_options(
                BuildCommand::Check,
                CompilationOptions::default(),
                entries,
                workspace.root_package_name(),
                false,
                false,
                true,
                false,
                None,
            )
        }
        PackageCommand::Bench => {
            let workspace = package::resolve(&invocation.root)
                .map_err(|error| CliError::io(error.to_string()))?;
            let lock_path = package::write_lockfile(&workspace)
                .map_err(|error| CliError::io(error.to_string()))?;
            println!("     Locked {}", lock_path.display());
            execute_plan_with_options(
                BuildCommand::Bench,
                CompilationOptions {
                    collect_metrics: true,
                    ..CompilationOptions::default()
                },
                workspace.source_entries(),
                workspace.root_package_name(),
                true,
                true,
                true,
                false,
                None,
            )
        }
        PackageCommand::Doc => {
            let workspace = package::resolve(&invocation.root)
                .map_err(|error| CliError::io(error.to_string()))?;
            let lock_path = package::write_lockfile(&workspace)
                .map_err(|error| CliError::io(error.to_string()))?;
            let docs_path =
                package::write_docs(&workspace).map_err(|error| CliError::io(error.to_string()))?;
            println!("     Locked {}", lock_path.display());
            println!("     Written docs {}", docs_path.display());
            Ok(())
        }
        PackageCommand::Add {
            name,
            version,
            path,
            registry,
        } => {
            let lock_path = package::add_dependency(
                &invocation.root,
                &name,
                version.as_deref(),
                path.as_deref(),
                registry.as_deref(),
            )
            .map_err(|error| CliError::io(error.to_string()))?;
            println!("     Added {}", name);
            println!("     Locked {}", lock_path.display());
            Ok(())
        }
        PackageCommand::Publish { registry } => {
            let package_path = package::publish(&invocation.root, &registry)
                .map_err(|error| CliError::io(error.to_string()))?;
            println!("     Published {}", package_path.display());
            Ok(())
        }
    }
}

fn execute_structured_diagnostics(
    entries: Vec<PathBuf>,
    mut options: CompilationOptions,
    sarif_output: bool,
) -> CliResult<()> {
    if entries.is_empty() {
        return Err(CliError::usage(
            "No Spectra source files were provided for linting.",
        ));
    }

    options.run_jit = false;
    run_structured_diagnostics(entries, options, sarif_output)
}

fn execute_repl_json(mut options: CompilationOptions, preload: Vec<PathBuf>) -> CliResult<()> {
    if preload.is_empty() {
        return Err(CliError::usage(
            "Provide one or more paths when using 'spectra repl --json'.",
        ));
    }

    configure_lint_options(&mut options, &preload, true, &[], &[])?;
    options.run_jit = false;
    run_structured_diagnostics(preload, options, false)
}

fn run_structured_diagnostics(
    entries: Vec<PathBuf>,
    options: CompilationOptions,
    sarif_output: bool,
) -> CliResult<()> {
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

            emit_diagnostic_report(&report, true, sarif_output)?;
            return Ok(());
        }
    };

    if plan.modules().is_empty() {
        let report = JsonDiagnosticReport {
            version: 1,
            success: true,
            files: Vec::new(),
        };
        emit_diagnostic_report(&report, false, sarif_output)?;
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

    emit_diagnostic_report(&report, has_errors, sarif_output)
}

fn emit_diagnostic_report(
    report: &JsonDiagnosticReport,
    has_errors: bool,
    sarif_output: bool,
) -> CliResult<()> {
    if sarif_output {
        emit_sarif_report(report, has_errors)
    } else {
        emit_json_report(report, has_errors)
    }
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

fn emit_sarif_report(report: &JsonDiagnosticReport, has_errors: bool) -> CliResult<()> {
    let mut rules: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut results = Vec::new();

    for file in &report.files {
        for diagnostic in &file.diagnostics {
            let rule_id = diagnostic
                .code
                .clone()
                .or_else(|| diagnostic.phase.clone())
                .unwrap_or_else(|| "diagnostic".to_string());
            rules.entry(rule_id.clone()).or_insert_with(|| {
                json!({
                    "id": rule_id,
                    "name": diagnostic.phase.clone().unwrap_or_else(|| "diagnostic".to_string()),
                    "shortDescription": { "text": diagnostic.message },
                    "help": { "text": diagnostic.hint.clone().unwrap_or_else(|| diagnostic.message.clone()) }
                })
            });

            let mut result = json!({
                "ruleId": rule_id,
                "level": diagnostic.severity.sarif_level(),
                "message": { "text": diagnostic.message },
                "locations": [
                    {
                        "physicalLocation": {
                            "artifactLocation": { "uri": file.path },
                            "region": {
                                "startLine": diagnostic.range.start.line,
                                "startColumn": diagnostic.range.start.column,
                                "endLine": diagnostic.range.end.line,
                                "endColumn": diagnostic.range.end.column
                            }
                        }
                    }
                ]
            });

            if let Some(hint) = &diagnostic.hint {
                result["properties"] = json!({ "hint": hint });
            }

            if !diagnostic.related.is_empty() {
                result["relatedLocations"] = json!(diagnostic
                    .related
                    .iter()
                    .enumerate()
                    .map(|(index, related)| {
                        let mut item = json!({
                            "id": index + 1,
                            "message": { "text": related.message }
                        });
                        if let Some(range) = &related.range {
                            item["physicalLocation"] = json!({
                                "artifactLocation": { "uri": file.path },
                                "region": {
                                    "startLine": range.start.line,
                                    "startColumn": range.start.column,
                                    "endLine": range.end.line,
                                    "endColumn": range.end.column
                                }
                            });
                        }
                        item
                    })
                    .collect::<Vec<_>>());
            }

            results.push(result);
        }
    }

    let report = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "SpectraLang",
                        "semanticVersion": env!("CARGO_PKG_VERSION"),
                        "informationUri": "https://github.com/spectralang/spectralang",
                        "rules": rules.into_values().collect::<Vec<_>>()
                    }
                },
                "results": results
            }
        ]
    });

    let mut stdout = io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &report).map_err(|error| {
        CliError::io(format!(
            "Failed to serialize diagnostics to SARIF: {}",
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
            span_error_to_json("lexical", e.code, e.message, e.span, e.context, e.hint)
        }
        CompilerError::Parse(e) => {
            span_error_to_json("parse", e.code, e.message, e.span, e.context, e.hint)
        }
        CompilerError::Semantic(e) => {
            span_error_to_json("semantic", e.code, e.message, e.span, e.context, e.hint)
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
    code: Option<String>,
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
        code: Some(code.unwrap_or_else(|| phase.to_string())),
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

impl JsonSeverity {
    fn sarif_level(&self) -> &'static str {
        match self {
            JsonSeverity::Error => "error",
            JsonSeverity::Warning => "warning",
        }
    }
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
    println!("    bench      Compile with benchmark timings and optional JSON report");
    println!("    repl       Start an interactive Spectra prompt");
    println!("    new        Scaffold a new Spectra project");
    println!("    package    Resolve, lock, build, publish, and consume packages");
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
    println!("    spectralang bench --bench-json target/bench.json src/");
    println!("    spectralang repl --run");
    println!("    spectralang new my-project");
    println!("    spectralang package build --root .");
    println!("    spectralang package add math --path ../math");
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
    println!("SpectraLang CLI - '{}' command", command.name());
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
        BuildCommand::Bench => {
            println!("    spectralang bench src/");
            println!("    spectralang bench --bench-json target/bench.json tests/validation/");
        }
    }
    println!();
    println!("Use 'spectralang --list-experimental' to see available experimental features.");
}

fn print_repl_help() {
    println!("SpectraLang CLI - 'repl' command");
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
    println!("SpectraLang CLI - 'new' command");
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

fn print_package_help() {
    println!("SpectraLang CLI - 'package' command");
    println!();
    println!("USAGE:");
    println!("    spectralang package <SUBCOMMAND> [OPTIONS]");
    println!();
    println!("SUBCOMMANDS:");
    println!("    lock       Resolve packages and write spectra.lock");
    println!("    build      Resolve, lock, and compile a package workspace");
    println!("    check      Resolve, lock, and type-check a package workspace");
    println!("    run        Resolve, lock, compile, and run the workspace entry point");
    println!("    test       Resolve, lock, and check package plus tests/ sources");
    println!("    bench      Resolve, lock, and check with pipeline timings");
    println!("    doc        Generate package documentation into target/spectra-docs");
    println!("    add        Add a path or registry dependency and refresh spectra.lock");
    println!("    update     Refresh spectra.lock from current manifests");
    println!("    publish    Publish the root package into a local registry directory");
    println!();
    println!("OPTIONS:");
    println!("    --root <path>          Package or workspace root (default: .)");
    println!("    --path <path>          Local dependency path for 'add'");
    println!("    --version <version>    Dependency version for 'add'");
    println!("    --registry <path>      Local registry path for 'add' or 'publish'");
    println!();
    println!("Examples:");
    println!("    spectralang package lock --root .");
    println!("    spectralang package build --root examples/workspace");
    println!("    spectralang package add math --path ../math --version 0.1.0");
    println!("    spectralang package publish --root packages/math --registry .spectra-registry");
    println!("    spectralang package add math --version 0.1.0 --registry .spectra-registry");
}

fn print_format_help() {
    println!("SpectraLang CLI - 'fmt' command");
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
    println!("SpectraLang CLI - 'lint' command");
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
    println!("    --json              Emit diagnostics as JSON");
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
    println!("    --json                 Emit diagnostics as JSON");
    println!("    --sarif                Emit diagnostics as SARIF 2.1.0");
    println!("    --bench-json <path>    Write benchmark timings as JSON (bench only)");
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
    let formatted = format!("{}\nUse 'spectra --help' for usage information.", trimmed);
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

    #[test]
    fn json_is_allowed_for_check() {
        let mut args = vec![
            "--json".to_string(),
            "../../tests/validation/60_pattern_control_surface.spectra".to_string(),
        ]
        .into_iter()
        .peekable();

        let invocation = parse_compilation_invocation(&mut args, BuildCommand::Check, false)
            .expect("check --json should parse");

        assert!(invocation.json_output);
        assert!(!invocation.sarif_output);
    }

    #[test]
    fn sarif_is_allowed_for_check() {
        let mut args = vec![
            "--sarif".to_string(),
            "../../tests/validation/60_pattern_control_surface.spectra".to_string(),
        ]
        .into_iter()
        .peekable();

        let invocation = parse_compilation_invocation(&mut args, BuildCommand::Check, false)
            .expect("check --sarif should parse");

        assert!(invocation.sarif_output);
        assert!(!invocation.json_output);
    }

    #[test]
    fn json_is_rejected_for_run() {
        let mut args = vec![
            "--json".to_string(),
            "../../tests/validation/60_pattern_control_surface.spectra".to_string(),
        ]
        .into_iter()
        .peekable();

        let error = parse_compilation_invocation(&mut args, BuildCommand::Run, false).unwrap_err();

        assert!(error.message.contains("--json"));
        assert_eq!(error.code.as_i32(), ExitCode::Usage.as_i32());
    }

    #[test]
    fn json_and_sarif_are_mutually_exclusive() {
        let mut args = vec![
            "--json".to_string(),
            "--sarif".to_string(),
            "../../tests/validation/60_pattern_control_surface.spectra".to_string(),
        ]
        .into_iter()
        .peekable();

        let error = parse_compilation_invocation(&mut args, BuildCommand::Check, false)
            .expect_err("json and sarif together should fail");

        assert!(error.message.contains("--json"));
        assert!(error.message.contains("--sarif"));
        assert_eq!(error.code.as_i32(), ExitCode::Usage.as_i32());
    }
}
