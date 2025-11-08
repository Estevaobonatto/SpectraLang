mod compiler_integration;
mod project;

use compiler_integration::SpectraCompiler;
use project::ProjectPlan;
use spectra_compiler::CompilationOptions;
use std::{env, fs, path::PathBuf, process};

const KNOWN_EXPERIMENTAL_FEATURES: &[&str] = &["switch", "unless", "do-while", "loop"];

#[derive(Debug)]
struct CliInvocation {
    entries: Vec<PathBuf>,
    options: CompilationOptions,
}

#[derive(Debug)]
enum CliAction {
    Help(Option<CommandKind>),
    ListExperimental,
    Command { kind: CommandKind, invocation: CliInvocation },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CommandKind {
    Compile,
    Check,
    Run,
}

impl CommandKind {
    fn name(self) -> &'static str {
        match self {
            CommandKind::Compile => "compile",
            CommandKind::Check => "check",
            CommandKind::Run => "run",
        }
    }

    fn description(self) -> &'static str {
        match self {
            CommandKind::Compile => "Compile Spectra modules (default).",
            CommandKind::Check => "Type-check modules and report diagnostics without executing.",
            CommandKind::Run => "Compile modules and execute the entry point via JIT.",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            CommandKind::Compile => "All files compiled successfully!",
            CommandKind::Check => "Check completed. No errors detected.",
            CommandKind::Run => "Compilation and execution finished successfully.",
        }
    }

    fn module_verb(self) -> &'static str {
        match self {
            CommandKind::Check => "Checking",
            CommandKind::Compile | CommandKind::Run => "Compiling",
        }
    }

    fn module_success_verb(self) -> &'static str {
        match self {
            CommandKind::Check => "checked",
            CommandKind::Compile | CommandKind::Run => "compiled",
        }
    }
}

fn main() {
    let action = match parse_cli() {
        Ok(action) => action,
        Err(message) => {
            eprintln!("{}", message);
            process::exit(1);
        }
    };

    match action {
        CliAction::Help(Some(command)) => {
            print_command_help(command);
        }
        CliAction::Help(None) => {
            print_global_help();
        }
        CliAction::ListExperimental => {
            print_experimental_features();
        }
        CliAction::Command { kind, invocation } => {
            if let Err(code) = execute_command(kind, invocation) {
                process::exit(code);
            }
        }
    }
}

fn parse_cli() -> Result<CliAction, String> {
    let mut args = env::args().skip(1).peekable();

    if args.peek().is_none() {
        return Err(usage_error("No command or input files provided."));
    }

    match args.peek().map(|value| value.as_str()) {
        Some("--help") | Some("-h") => {
            args.next();
            return Ok(CliAction::Help(None));
        }
        Some("help") => {
            args.next();
            if let Some(target) = args.next() {
                if let Some(kind) = parse_command_name(&target) {
                    return Ok(CliAction::Help(Some(kind)));
                } else {
                    return Err(usage_error(&format!("Unknown command '{}'.", target)));
                }
            } else {
                return Ok(CliAction::Help(None));
            }
        }
        Some("--list-experimental") => {
            args.next();
            if args.peek().is_some() {
                return Err(usage_error(
                    "--list-experimental must be used on its own.",
                ));
            }
            return Ok(CliAction::ListExperimental);
        }
        _ => {}
    }

    let mut command = CommandKind::Compile;

    if let Some(value) = args.peek() {
        if !value.starts_with('-') {
            if let Some(kind) = parse_command_name(value) {
                command = kind;
                args.next();
            }
        }
    }

    if let Some(flag) = args.peek() {
        if matches!(flag.as_str(), "--help" | "-h") {
            args.next();
            return Ok(CliAction::Help(Some(command)));
        }
    }

    let invocation = parse_compilation_invocation(&mut args, command)?;

    Ok(CliAction::Command {
        kind: command,
        invocation,
    })
}

fn parse_command_name(value: &str) -> Option<CommandKind> {
    match value {
        "compile" | "build" => Some(CommandKind::Compile),
        "check" => Some(CommandKind::Check),
        "run" => Some(CommandKind::Run),
        _ => None,
    }
}

fn parse_compilation_invocation<I>(
    args: &mut std::iter::Peekable<I>,
    command: CommandKind,
) -> Result<CliInvocation, String>
where
    I: Iterator<Item = String>,
{
    let mut options = CompilationOptions::default();
    let mut entries = Vec::new();

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
            "--timings" | "-T" => options.collect_metrics = true,
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
                if command == CommandKind::Check {
                    return Err(usage_error(
                        "'--run' cannot be used with the 'check' command.",
                    ));
                }
                options.run_jit = true;
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
        return Err(usage_error(
            "No source files or directories were provided.",
        ));
    }

    match command {
        CommandKind::Run => options.run_jit = true,
        CommandKind::Check => options.run_jit = false,
        CommandKind::Compile => {}
    }

    Ok(CliInvocation { entries, options })
}

fn execute_command(kind: CommandKind, invocation: CliInvocation) -> Result<(), i32> {
    let CliInvocation { entries, options } = invocation;

    let plan = match ProjectPlan::build(entries) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("{}", error);
            return Err(1);
        }
    };

    if plan.modules().is_empty() {
        eprintln!("No Spectra source files found to compile.");
        return Err(1);
    }

    let mut compiler = SpectraCompiler::new(options);
    let mut has_failures = false;

    for module in plan.modules() {
        println!(
            "\n{} module: {} ({})",
            kind.module_verb(),
            module.name,
            module.path.display()
        );

        let filename = module.path.to_string_lossy().to_string();
        match fs::read_to_string(&module.path) {
            Ok(source) => match compiler.compile(&source, &filename) {
                Ok(()) => {
                    println!(
                        "\nSuccessfully {} module '{}'",
                        kind.module_success_verb(),
                        module.name
                    );
                }
                Err(error) => {
                    has_failures = true;
                    eprintln!(
                        "\nCompilation failed for module '{}' ({})",
                        module.name,
                        module.path.display()
                    );
                    eprintln!("{}", error);
                }
            },
            Err(error) => {
                has_failures = true;
                eprintln!(
                    "\nFailed to read file for module '{}' ({}):",
                    module.name,
                    module.path.display()
                );
                eprintln!("Error: {}", error);
            }
        }
    }

    compiler.print_aggregate_summary();

    if has_failures {
        eprintln!("\n💥 Command '{}' completed with errors", kind.name());
        Err(1)
    } else {
        println!("\n{}", kind.success_message());
        Ok(())
    }
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
    println!("    spectra --list-experimental");
    println!();
    print_experimental_features();
}

fn print_command_help(command: CommandKind) {
    println!("SpectraLang CLI – '{}' command", command.name());
    println!();
    println!("USAGE:");
    println!(
        "    spectra {} [OPTIONS] <paths>...",
        command.name()
    );
    println!();
    println!("{}", command.description());
    println!();
    print_compilation_options(Some(command));
    println!();
    println!("Examples:");
    match command {
        CommandKind::Compile => {
            println!("    spectra compile src/main.spectra");
            println!("    spectra compile --dump-ir project/");
        }
        CommandKind::Check => {
            println!("    spectra check src/");
            println!("    spectra check --dump-ast main.spectra");
        }
        CommandKind::Run => {
            println!("    spectra run app.spectra");
            println!("    spectra run --timings src/main.spectra");
        }
    }
    println!();
    println!("Use 'spectra --list-experimental' to see available experimental features.");
}

fn print_compilation_options(command: Option<CommandKind>) {
    println!("COMPILATION OPTIONS:");
    println!("    --dump-ast             Print the AST for debugging");
    println!("    --dump-ir              Print the IR for debugging");
    println!("    --timings, -T          Report compilation and execution timings");
    println!("    --no-optimize, -O0     Disable all optimizations");
    println!("    -O1                    Enable basic optimizations");
    println!("    -O2                    Enable moderate optimizations (default)");
    println!("    -O3                    Enable aggressive optimizations");
    match command {
        Some(CommandKind::Check) => {
            println!("    --run, -r              Not available for the 'check' command");
        }
        Some(CommandKind::Run) => {
            println!("    --run, -r              Redundant; 'run' always executes after compiling");
        }
        _ => {
            println!(
                "    --run, -r              Execute the program with the JIT after compiling"
            );
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

fn usage_error(message: &str) -> String {
    let trimmed = message.trim_end();
    format!(
        "{}\nUse 'spectra --help' for usage information.",
        trimmed
    )
}
