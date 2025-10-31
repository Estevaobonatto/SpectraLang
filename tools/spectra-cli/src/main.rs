use std::{
    fs, io,
    path::{Path, PathBuf},
    process::exit,
};

use clap::{Args, Parser as ClapParser, Subcommand};
use spectra_compiler::{
    ast::Module,
    lexer::Lexer,
    parser::Parser,
    project::{find_console_entry_point, EntryPointError},
    semantic,
};

#[derive(ClapParser, Debug)]
#[command(name = "spectra", about = "SpectraLang CLI prototype", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Build a SpectraLang console project.
    Build(BuildArgs),
    /// Build a project and (future) execute the resulting binary.
    Run(RunArgs),
    /// Scaffold a new console project with a simple main function.
    New(NewArgs),
}

#[derive(Args, Debug, Clone)]
struct BuildArgs {
    /// Project directory containing a `src/` tree.
    #[arg(value_name = "PATH", default_value = ".")]
    project: PathBuf,
    /// Override the binary name (default: sanitized project folder name).
    #[arg(long, value_name = "NAME")]
    bin: Option<String>,
    /// Fully-qualified module name que contém `fn main()`.
    #[arg(long, value_name = "MODULE")]
    main: Option<String>,
    /// Use release profile output directory (`target/release`).
    #[arg(long)]
    release: bool,
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Project directory containing a `src/` tree.
    #[arg(value_name = "PATH", default_value = ".")]
    project: PathBuf,
    /// Override the binary name (default: sanitized project folder name).
    #[arg(long, value_name = "NAME")]
    bin: Option<String>,
    /// Fully-qualified module name que contém `fn main()`.
    #[arg(long, value_name = "MODULE")]
    main: Option<String>,
    /// Use release profile output directory (`target/release`).
    #[arg(long)]
    release: bool,
    /// Arguments to pass to the SpectraLang program (reserved for future backend integration).
    #[arg(trailing_var_arg = true, value_name = "ARGS")]
    args: Vec<String>,
}

#[derive(Args, Debug)]
struct NewArgs {
    /// Folder name for the new project.
    #[arg(value_name = "NAME")]
    name: String,
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Build(args) => build_command(args),
        Command::Run(args) => run_command(args),
        Command::New(args) => new_command(args),
    };

    if let Err(code) = exit_code {
        exit(code);
    }
}

fn build_command(args: BuildArgs) -> Result<(), i32> {
    let output = execute_build(&args)?;
    println!(
        "Built '{}' ({}) from {} source(s); entry module '{}'.\n  artifact: {}",
        output.bin_name,
        output.profile.dir_name(),
        output.source_count,
        output.main_module_name,
        output.artifact_path.display()
    );
    Ok(())
}

fn run_command(args: RunArgs) -> Result<(), i32> {
    let forwarded_args = args.args;

    let build_args = BuildArgs {
        project: args.project,
        bin: args.bin,
        main: args.main,
        release: args.release,
    };

    let output = execute_build(&build_args)?;
    println!(
        "Build ready at {} (binary '{}').",
        output.artifact_path.display(),
        output.bin_name
    );
    if !forwarded_args.is_empty() {
        println!(
            "Note: runtime execution is not implemented yet; arguments {:?} were captured for future use.",
            forwarded_args
        );
    } else {
        println!("Runtime execution is not implemented yet.");
    }
    Ok(())
}

fn new_command(args: NewArgs) -> Result<(), i32> {
    let root = PathBuf::from(&args.name);
    if root.exists() {
        eprintln!("error: directory '{}' already exists", root.display());
        return Err(1);
    }

    fs::create_dir_all(root.join("src")).map_err(|err| {
        eprintln!(
            "error: failed to create project directories for '{}': {}",
            args.name, err
        );
        2
    })?;

    let module_segment = sanitize_identifier(&args.name);
    let module_segment = if module_segment.is_empty() {
        "app".to_string()
    } else {
        module_segment
    };

    let main_contents = format!(
        "module {}.main;\n\nfn main(): i32 {{\n    return 0;\n}}\n",
        module_segment
    );

    fs::write(root.join("src/main.spc"), main_contents).map_err(|err| {
        eprintln!(
            "error: failed to write main module for '{}': {}",
            args.name, err
        );
        2
    })?;

    let readme_contents = format!(
        "# {}\n\nProjeto gerado com `spectra new {}`.\n\nComandos sugeridos:\n- `spectra build` — compila a aplicação de console.\n- `spectra run` — compila (e futuramente executa) a aplicação.\n",
        args.name, args.name
    );

    fs::write(root.join("README.md"), readme_contents).map_err(|err| {
        eprintln!("error: failed to write README: {}", err);
        2
    })?;

    fs::write(root.join(".gitignore"), "target/\n").map_err(|err| {
        eprintln!("error: failed to write .gitignore: {}", err);
        2
    })?;

    println!(
        "Created SpectraLang console project '{}' at {}",
        args.name,
        PathBuf::from(&args.name).display()
    );

    Ok(())
}

fn execute_build(args: &BuildArgs) -> Result<BuildOutput, i32> {
    let project_root = canonical_project_root(&args.project)?;
    let sources = discover_sources(&project_root)?;
    if sources.is_empty() {
        eprintln!(
            "error: no SpectraLang sources found under '{}'",
            project_root.join("src").display()
        );
        return Err(1);
    }

    let modules = compile_sources(&sources)?;
    let module_refs: Vec<&Module> = modules.iter().map(|(_, module)| module).collect();
    let entry_point = match find_console_entry_point(&module_refs, args.main.as_deref()) {
        Ok(entry) => entry,
        Err(errors) => {
            report_entrypoint_errors(errors);
            return Err(4);
        }
    };

    let bin_name = derive_bin_name(args.bin.as_deref(), &project_root);
    let profile = if args.release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    };

    let artifact_path = write_manifest(
        &project_root,
        profile,
        &bin_name,
        entry_point.module,
        &sources,
    )?;

    Ok(BuildOutput {
        artifact_path,
        source_count: sources.len(),
        bin_name,
        profile,
        main_module_name: module_name(entry_point.module),
    })
}

fn compile_sources(source_files: &[PathBuf]) -> Result<Vec<(PathBuf, Module)>, i32> {
    let mut modules = Vec::new();

    for path in source_files {
        let source = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(err) => {
                eprintln!("error: failed to read '{}': {}", path.display(), err);
                return Err(2);
            }
        };

        let tokens = match Lexer::new(&source).tokenize() {
            Ok(tokens) => tokens,
            Err(errors) => {
                eprintln!("lexical error(s) in {}:", path.display());
                for error in errors {
                    eprintln!("  - {}", error);
                }
                return Err(3);
            }
        };

        let module = match Parser::new(&tokens).parse() {
            Ok(module) => module,
            Err(errors) => {
                eprintln!("parse error(s) in {}:", path.display());
                for error in errors {
                    eprintln!("  - {}", error);
                }
                return Err(3);
            }
        };

        modules.push((path.clone(), module));
    }

    let module_refs: Vec<&Module> = modules.iter().map(|(_, module)| module).collect();
    if let Err(errors) = semantic::analyze_modules(&module_refs) {
        eprintln!("semantic error(s):");
        for error in errors {
            eprintln!("  - {}", error);
        }
        return Err(4);
    }

    Ok(modules)
}

fn canonical_project_root(path: &Path) -> Result<PathBuf, i32> {
    if !path.exists() {
        eprintln!(
            "error: project directory '{}' does not exist",
            path.display()
        );
        return Err(1);
    }
    if !path.is_dir() {
        eprintln!(
            "error: project path '{}' is not a directory",
            path.display()
        );
        return Err(1);
    }
    match path.canonicalize() {
        Ok(canonical) => Ok(canonical),
        Err(err) => {
            eprintln!(
                "error: failed to resolve project directory '{}': {}",
                path.display(),
                err
            );
            Err(2)
        }
    }
}

fn discover_sources(project_root: &Path) -> Result<Vec<PathBuf>, i32> {
    let src_dir = project_root.join("src");
    if !src_dir.exists() {
        eprintln!(
            "error: project is missing 'src/' directory at '{}'",
            src_dir.display()
        );
        return Err(1);
    }

    let mut sources = Vec::new();
    gather_sources(&src_dir, &mut sources).map_err(|err| {
        eprintln!("error: failed to scan '{}': {}", src_dir.display(), err);
        2
    })?;
    sources.sort();
    Ok(sources)
}

fn gather_sources(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            gather_sources(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "spc") {
            files.push(path);
        }
    }
    Ok(())
}

fn write_manifest(
    project_root: &Path,
    profile: BuildProfile,
    bin_name: &str,
    main_module: &Module,
    sources: &[PathBuf],
) -> Result<PathBuf, i32> {
    let target_dir = project_root.join("target").join(profile.dir_name());
    fs::create_dir_all(&target_dir).map_err(|err| {
        eprintln!(
            "error: failed to create target directory '{}': {}",
            target_dir.display(),
            err
        );
        2
    })?;

    let artifact_path = target_dir.join(format!("{bin_name}.build.txt"));

    let mut contents = String::new();
    contents.push_str("# SpectraLang build metadata\n");
    contents.push_str(&format!("binary = {bin_name}\n"));
    contents.push_str(&format!("profile = {}\n", profile.dir_name()));
    contents.push_str(&format!("main_module = {}\n", module_name(main_module)));
    contents.push_str("sources = [\n");
    for path in sources {
        let relative = path.strip_prefix(project_root).unwrap_or(path);
        contents.push_str(&format!("  \"{}\",\n", relative.display()));
    }
    contents.push_str("]\n");

    fs::write(&artifact_path, contents).map_err(|err| {
        eprintln!(
            "error: failed to write build artifact '{}': {}",
            artifact_path.display(),
            err
        );
        2
    })?;

    Ok(artifact_path)
}

fn report_entrypoint_errors(errors: Vec<EntryPointError>) {
    eprintln!("entry point error(s):");
    for error in errors {
        if let Some(span) = error.span {
            eprintln!(
                "  - {} (line {}, column {})",
                error.message, span.start_location.line, span.start_location.column
            );
        } else {
            eprintln!("  - {}", error.message);
        }
    }
}

fn module_name(module: &Module) -> String {
    module
        .name
        .as_ref()
        .map(|path| path.segments.join("."))
        .unwrap_or_else(|| "<anonymous>".to_string())
}

fn derive_bin_name(explicit: Option<&str>, project_root: &Path) -> String {
    if let Some(name) = explicit {
        let sanitized = sanitize_identifier(name);
        return if sanitized.is_empty() {
            "app".to_string()
        } else {
            sanitized
        };
    }

    let fallback = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app");
    let sanitized = sanitize_identifier(fallback);
    if sanitized.is_empty() {
        "app".to_string()
    } else {
        sanitized
    }
}

fn sanitize_identifier(value: &str) -> String {
    let filtered: String = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            '_' | '-' => '_',
            _ => '_',
        })
        .collect();

    filtered.trim_matches('_').to_string()
}

#[derive(Debug)]
struct BuildOutput {
    artifact_path: PathBuf,
    source_count: usize,
    bin_name: String,
    profile: BuildProfile,
    main_module_name: String,
}

#[derive(Debug, Clone, Copy)]
enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn dir_name(self) -> &'static str {
        match self {
            BuildProfile::Debug => "debug",
            BuildProfile::Release => "release",
        }
    }
}
