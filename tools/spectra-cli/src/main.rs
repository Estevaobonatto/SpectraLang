mod compiler_integration;
mod project;

use compiler_integration::{ModulePipelineSummary, SpectraCompiler};
use project::ProjectPlan;
use spectra_compiler::CompilationOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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
}

#[derive(Debug)]
struct ReplOptions {
    base_options: CompilationOptions,
    preload: Vec<PathBuf>,
    autorun: bool,
    show_pipeline_summary: bool,
    verbose: bool,
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum HelpTopic {
    Global,
    Build(BuildCommand),
    Repl,
    NewProject,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BuildCommand {
    Compile,
    Check,
    Run,
}

impl BuildCommand {
    fn name(self) -> &'static str {
        match self {
            BuildCommand::Compile => "compile",
            BuildCommand::Check => "check",
            BuildCommand::Run => "run",
        }
    }

    fn description(self) -> &'static str {
        match self {
            BuildCommand::Compile => "Compile Spectra modules (default).",
            BuildCommand::Check => "Type-check modules and report diagnostics without executing.",
            BuildCommand::Run => "Compile modules and execute the entry point via JIT.",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            BuildCommand::Compile => "All files compiled successfully!",
            BuildCommand::Check => "Check completed. No errors detected.",
            BuildCommand::Run => "Compilation and execution finished successfully.",
        }
    }

    fn module_verb(self) -> &'static str {
        match self {
            BuildCommand::Check => "Checking",
            BuildCommand::Compile | BuildCommand::Run => "Compiling",
        }
    }

    fn module_success_verb(self) -> &'static str {
        match self {
            BuildCommand::Check => "checked",
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

    let invocation = parse_compilation_invocation(&mut args, command)?;

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
        _ => None,
    }
}

fn parse_compilation_invocation<I>(
    args: &mut std::iter::Peekable<I>,
    command: BuildCommand,
) -> CliResult<CliInvocation>
where
    I: Iterator<Item = String>,
{
    let mut options = CompilationOptions::default();
    let mut entries = Vec::new();
    let mut show_pipeline_summary = false;
    let mut verbose = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" => {
                for remaining in args {
                    entries.push(PathBuf::from(remaining));
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
        BuildCommand::Check => options.run_jit = false,
        BuildCommand::Compile => {}
    }

    Ok(CliInvocation {
        entries,
        options,
        show_pipeline_summary,
        verbose,
    })
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

fn execute_build_command(kind: BuildCommand, invocation: CliInvocation) -> CliResult<()> {
    let CliInvocation {
        entries,
        options,
        show_pipeline_summary,
        verbose,
    } = invocation;

    execute_plan_with_options(
        kind,
        options,
        entries,
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
    let mut has_failures = false;

    for module in plan.modules() {
        println!(
            "\n{} module: {} ({})",
            kind.module_verb(),
            module.name,
            module.path.display()
        );

        if verbose {
            if module.imports.is_empty() {
                println!("    imports: (none)");
            } else {
                println!("    imports: {}", module.imports.join(", "));
            }
        }

        let filename = module.path.to_string_lossy().to_string();
        match fs::read_to_string(&module.path) {
            Ok(source) => match compiler.compile(&source, &filename) {
                Ok(()) => {
                    println!(
                        "\nSuccessfully {} module '{}'",
                        kind.module_success_verb(),
                        module.name
                    );
                    if show_pipeline_summary {
                        if let Some(summary) = compiler.take_last_summary() {
                            print_pipeline_summary(&summary);
                        }
                    }
                }
                Err(error) => {
                    has_failures = true;
                    println!();
                    log_error(&format!(
                        "Compilation failed for module '{}' ({})\n{}",
                        module.name,
                        module.path.display(),
                        error
                    ));
                }
            },
            Err(error) => {
                has_failures = true;
                println!();
                log_error(&format!(
                    "Failed to read file for module '{}' ({})\nError: {}",
                    module.name,
                    module.path.display(),
                    error
                ));
            }
        }
    }

    has_failures
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

    if show_pipeline_summary {
        compiler.set_emit_internal_metrics(false);
    }

    let has_failures = compile_plan(kind, &mut compiler, &plan, show_pipeline_summary, verbose);

    if show_aggregate_summary {
        compiler.print_aggregate_summary();
    }

    if has_failures {
        println!();
        Err(CliError::compilation(format!(
            "💥 Command '{}' completed with errors",
            kind.name()
        )))
    } else {
        if print_success {
            println!("\n{}", kind.success_message());
        }
        Ok(())
    }
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
    } = options;

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
            BuildCommand::Check => options.run_jit = false,
            BuildCommand::Compile => options.run_jit = false,
        }

        execute_plan_with_options(
            command,
            options,
            entries,
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
    let manifest_path = path.join("Spectra.toml");
    let main_source_path = path.join("src").join("main.spectra");

    let manifest_contents = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\n\n[build]\nentry = \"src/main.spectra\"\n",
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
    println!("    spectra <COMMAND> [OPTIONS] <paths>...");
    println!();
    println!("COMMANDS:");
    println!("    compile    Compile Spectra modules (default)");
    println!("    check      Type-check modules and report diagnostics");
    println!("    run        Compile modules and execute the entry point via JIT");
    println!("    repl       Start an interactive Spectra prompt");
    println!("    new        Scaffold a new Spectra project");
    println!("    help       Print this help message");
    println!();
    println!("GLOBAL OPTIONS:");
    println!("    -h, --help             Print this help message");
    println!("    --list-experimental    List available experimental features and exit");
    println!();
    print_compilation_options(None);
    println!();
    println!("EXAMPLES:");
    println!("    spectra compile src/main.spectra");
    println!("    spectra check examples/");
    println!("    spectra run -O3 app.spectra");
    println!("    spectra repl --run");
    println!("    spectra new my-project");
    println!("    spectra --list-experimental");
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
    println!("    spectra {} [OPTIONS] <paths>...", command.name());
    println!();
    println!("{}", command.description());
    println!();
    print_compilation_options(Some(command));
    println!();
    println!("Examples:");
    match command {
        BuildCommand::Compile => {
            println!("    spectra compile src/main.spectra");
            println!("    spectra compile --dump-ir project/");
        }
        BuildCommand::Check => {
            println!("    spectra check src/");
            println!("    spectra check --dump-ast main.spectra");
        }
        BuildCommand::Run => {
            println!("    spectra run app.spectra");
            println!("    spectra run --timings src/main.spectra");
        }
    }
    println!();
    println!("Use 'spectra --list-experimental' to see available experimental features.");
}

fn print_repl_help() {
    println!("SpectraLang CLI – 'repl' command");
    println!();
    println!("USAGE:");
    println!("    spectra repl [OPTIONS] [paths]...");
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
    println!("    spectra new [OPTIONS] <path>");
    println!();
    println!("Create a new Spectra project with a starter module and manifest.");
    println!();
    println!("OPTIONS:");
    println!("    -f, --force        Scaffold even if the directory already exists");
    println!();
    println!("Examples:");
    println!("    spectra new hello-world");
    println!("    spectra new --force .");
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
        Some(BuildCommand::Check) => {
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
}
