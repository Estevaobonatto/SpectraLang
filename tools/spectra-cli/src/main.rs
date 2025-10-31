use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    process::exit,
};

use clap::{Args, Parser as ClapParser, Subcommand};
use spectra_compiler::{
    ast::Module,
    lexer::Lexer,
    parser::Parser,
    project::{collect_console_entry_points, EntryPoint, EntryPointError},
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
    /// Build all entry points found no projeto, gerando um artefato por módulo `main`.
    #[arg(long)]
    all: bool,
    /// Use release profile output directory (`target/release`).
    #[arg(long)]
    release: bool,
}

#[derive(Args, Debug, Clone)]
struct RunArgs {
    /// Project directory containing a `src/` tree.
    #[arg(value_name = "PATH", default_value = ".")]
    project: PathBuf,
    /// Override the binary name.
    #[arg(long, value_name = "NAME")]
    bin: Option<String>,
    /// Fully-qualified module name que contém `fn main()`.
    #[arg(long, value_name = "MODULE")]
    main: Option<String>,
    /// Use release profile output directory (`target/release`).
    #[arg(long)]
    release: bool,
    /// Arguments forwarded to the compiled binary (execution TBD).
    #[arg(trailing_var_arg = true, value_name = "ARGS")]
    args: Vec<String>,
}

#[derive(Args, Debug, Clone)]
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
    if output.artifacts.len() == 1 {
        let artifact = &output.artifacts[0];
        println!(
            "Built '{}' ({}) from {} source(s); entry module '{}'.\n  artifact: {}",
            artifact.bin_name,
            output.profile.dir_name(),
            output.source_count,
            artifact.main_module_name,
            artifact.artifact_path.display()
        );
    } else {
        println!(
            "Built {} console binaries (profile {}) from {} source(s):",
            output.artifacts.len(),
            output.profile.dir_name(),
            output.source_count
        );
        for artifact in &output.artifacts {
            println!(
                "  - module '{}' -> '{}' ({})",
                artifact.main_module_name,
                artifact.bin_name,
                artifact.artifact_path.display()
            );
        }
    }
    Ok(())
}

fn run_command(args: RunArgs) -> Result<(), i32> {
    let forwarded_args = args.args;

    let build_args = BuildArgs {
        project: args.project,
        bin: args.bin,
        main: args.main,
        all: false,
        release: args.release,
    };

    let output = execute_build(&build_args)?;
    let Some(artifact) = output.primary() else {
        eprintln!("error: build produced no artifacts");
        return Err(1);
    };

    if output.artifacts.len() != 1 {
        eprintln!(
            "error: `spectra run` requires a single entry point; use `--main <module>` to pick one."
        );
        return Err(1);
    }
    println!(
        "Build ready at {} (binary '{}').",
        artifact.artifact_path.display(),
        artifact.bin_name
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

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).map_err(|err| {
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
        r#"module {}.main;

import std.console;

fn main(): i32 {{
    println("Hello from SpectraLang!");
    return 0;
}}
"#,
        module_segment
    );

    fs::write(src_dir.join("main.spc"), main_contents).map_err(|err| {
        eprintln!(
            "error: failed to write main module for '{}': {}",
            args.name, err
        );
        2
    })?;

    let std_dir = src_dir.join("std");
    fs::create_dir_all(&std_dir).map_err(|err| {
        eprintln!(
            "error: failed to create std module directory for '{}': {}",
            args.name, err
        );
        2
    })?;

    let console_stub = "module std.console;\n\npub fn print(_message: string): void {\n    return;\n}\n\npub fn println(_message: string): void {\n    return;\n}\n\npub fn print_err(_message: string): void {\n    return;\n}\n\npub fn println_err(_message: string): void {\n    return;\n}\n\npub fn read_line(): string {\n    return \"\";\n}\n";

    fs::write(std_dir.join("console.spc"), console_stub).map_err(|err| {
        eprintln!(
            "error: failed to write std.console stub for '{}': {}",
            args.name, err
        );
        2
    })?;

    let readme_contents = format!(
        "# {}\n\nProjeto gerado com `spectra new {}`.\n\nEste template importa `std.console` e inclui um stub em `src/std/console.spc` até que o runtime esteja conectado. Explore `docs/console-io-recipes.md` para padrões de entrada/saída.\n\nComandos sugeridos:\n- `spectra build` — compila a aplicação de console.\n- `spectra run` — compila (e futuramente executa) a aplicação.\n",
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
    let all_entry_points = match collect_console_entry_points(&module_refs) {
        Ok(entries) => entries,
        Err(errors) => {
            report_entrypoint_errors(errors);
            return Err(4);
        }
    };

    if all_entry_points.is_empty() {
        report_entrypoint_errors(vec![EntryPointError::new(
            "no entry point found; define `fn main(): i32 { ... }`",
            None,
        )]);
        return Err(4);
    }

    let module_matches = |entry: &EntryPoint, target: &str| {
        entry
            .module
            .name
            .as_ref()
            .map(|path| path.segments.join("."))
            .map(|name| name == target)
            .unwrap_or(false)
    };

    let selected_entries: Vec<EntryPoint> = if args.all {
        if let Some(target) = args.main.as_deref() {
            let filtered: Vec<EntryPoint> = all_entry_points
                .iter()
                .copied()
                .filter(|entry| module_matches(entry, target))
                .collect();
            if filtered.is_empty() {
                report_entrypoint_errors(vec![EntryPointError::new(
                    format!(
                        "no entry point `fn main` found in module '{}' (available: {})",
                        target,
                        format_entry_point_list(&all_entry_points)
                    ),
                    None,
                )]);
                return Err(4);
            }
            filtered
        } else {
            all_entry_points.clone()
        }
    } else if let Some(target) = args.main.as_deref() {
        if let Some(entry) = all_entry_points
            .iter()
            .copied()
            .find(|entry| module_matches(entry, target))
        {
            vec![entry]
        } else {
            report_entrypoint_errors(vec![EntryPointError::new(
                format!(
                    "no entry point `fn main` found in module '{}' (available: {})",
                    target,
                    format_entry_point_list(&all_entry_points)
                ),
                None,
            )]);
            return Err(4);
        }
    } else if all_entry_points.len() == 1 {
        vec![all_entry_points[0]]
    } else {
        eprintln!(
            "error: multiple entry points found ({}). Use `--main <module>` or `--all`.",
            format_entry_point_list(&all_entry_points)
        );
        return Err(4);
    };

    let profile = if args.release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    };

    let bin_entries = assign_bin_names(&selected_entries, args, &project_root);
    let module_bin_pairs: Vec<(String, String)> = bin_entries
        .iter()
        .map(|(entry, bin)| (module_name(entry.module), bin.clone()))
        .collect();

    let mut artifacts = Vec::new();
    for (entry, bin_name) in bin_entries {
        let artifact_path = write_manifest(
            &project_root,
            profile,
            &bin_name,
            entry,
            &sources,
            &module_bin_pairs,
        )?;
        artifacts.push(BinaryArtifact {
            artifact_path,
            bin_name,
            main_module_name: module_name(entry.module),
        });
    }

    Ok(BuildOutput {
        artifacts,
        source_count: sources.len(),
        profile,
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
    entry_point: EntryPoint<'_>,
    sources: &[PathBuf],
    bin_map: &[(String, String)],
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
    contents.push_str(&format!(
        "main_module = {}\n",
        module_name(entry_point.module)
    ));
    contents.push_str("binaries = {\n");
    for (module, mapped_bin) in bin_map {
        contents.push_str(&format!("  {} = \"{}\"\n", module, mapped_bin));
    }
    contents.push_str("}\n");
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

fn format_entry_point_list(entries: &[EntryPoint]) -> String {
    if entries.is_empty() {
        return "<nenhum>".to_string();
    }

    let mut names: Vec<String> = entries
        .iter()
        .map(|entry| module_name(entry.module))
        .collect();
    names.sort();
    names.dedup();
    names.join(", ")
}

fn assign_bin_names<'m>(
    entries: &[EntryPoint<'m>],
    args: &BuildArgs,
    project_root: &Path,
) -> Vec<(EntryPoint<'m>, String)> {
    let mut bins = Vec::new();
    if entries.is_empty() {
        return bins;
    }

    if !args.all {
        let bin_name = derive_bin_name(args.bin.as_deref(), project_root);
        bins.push((entries[0], bin_name));
        return bins;
    }

    let base_override = args
        .bin
        .as_deref()
        .map(|name| sanitize_identifier(name))
        .filter(|name| !name.is_empty());

    let mut used = HashSet::new();
    for (index, entry) in entries.iter().copied().enumerate() {
        let mut candidate = sanitize_identifier(&module_name(entry.module));
        if candidate.is_empty() {
            candidate = format!("bin{}", index + 1);
        }
        if let Some(base) = &base_override {
            candidate = format!("{}_{}", base, candidate);
        }
        if candidate.is_empty() {
            candidate = format!("bin{}", index + 1);
        }

        let mut final_name = candidate.clone();
        let mut attempt = 2;
        while !used.insert(final_name.clone()) {
            final_name = format!("{}_{}", candidate, attempt);
            attempt += 1;
        }

        bins.push((entry, final_name));
    }

    bins
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
    artifacts: Vec<BinaryArtifact>,
    source_count: usize,
    profile: BuildProfile,
}

impl BuildOutput {
    fn primary(&self) -> Option<&BinaryArtifact> {
        self.artifacts.first()
    }
}

#[derive(Debug)]
struct BinaryArtifact {
    artifact_path: PathBuf,
    bin_name: String,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    use tempfile::TempDir;

    struct DirGuard {
        previous: PathBuf,
    }

    impl DirGuard {
        fn change_to(path: &Path) -> Self {
            let previous = std::env::current_dir().expect("cwd");
            std::env::set_current_dir(path).expect("set cwd");
            Self { previous }
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }

    #[test]
    fn build_single_entry_produces_manifest_with_mapping() {
        let project = sample_project(&[(
            "main.spc",
            "module demo.main;\n\nfn main(): i32 {\n    return 0;\n}\n",
        )]);

        let args = BuildArgs {
            project: project.path().to_path_buf(),
            bin: None,
            main: None,
            all: false,
            release: false,
        };

        let output = execute_build(&args).expect("build ok");
        assert_eq!(output.artifacts.len(), 1);
        let artifact = output.primary().expect("artifact present");
        let manifest = fs::read_to_string(&artifact.artifact_path).expect("manifest readable");
        assert!(manifest.contains("main_module = demo.main"));
        assert!(manifest.contains("binaries = {"));
        assert!(manifest.contains("demo.main"));
    }

    #[test]
    fn build_all_generates_multiple_artifacts() {
        let project = sample_project(&[
            (
                "alpha.spc",
                "module demo.alpha;\n\nfn main(): i32 {\n    return 0;\n}\n",
            ),
            (
                "beta.spc",
                "module demo.beta;\n\nfn main(): i32 {\n    return 1;\n}\n",
            ),
        ]);

        let args = BuildArgs {
            project: project.path().to_path_buf(),
            bin: None,
            main: None,
            all: true,
            release: false,
        };

        let output = execute_build(&args).expect("build ok");
        assert_eq!(output.artifacts.len(), 2);
        for artifact in &output.artifacts {
            let manifest = fs::read_to_string(&artifact.artifact_path).expect("manifest readable");
            assert!(manifest.contains("demo.alpha"));
            assert!(manifest.contains("demo.beta"));
        }
    }

    #[test]
    fn new_command_generates_console_template() {
        let tmp = TempDir::new().expect("temp dir");
        let _guard = DirGuard::change_to(tmp.path());

        new_command(NewArgs {
            name: "demo_app".to_string(),
        })
        .expect("new ok");

        let project_root = tmp.path().join("demo_app");
        let main_source =
            fs::read_to_string(project_root.join("src/main.spc")).expect("main.spc readable");
        assert!(main_source.contains("import std.console;"));
        assert!(main_source.contains("println(\"Hello from SpectraLang!\")"));

        let console_source = fs::read_to_string(project_root.join("src/std/console.spc"))
            .expect("console stub readable");
        assert!(console_source.contains("module std.console;"));
        assert!(console_source.contains("pub fn println"));

        let readme = fs::read_to_string(project_root.join("README.md")).expect("readme readable");
        assert!(readme.contains("std.console"));
    }

    fn sample_project(files: &[(&str, &str)]) -> TempDir {
        let tmp = TempDir::new().expect("temp dir");
        let src_dir = tmp.path().join("src");
        fs::create_dir(&src_dir).expect("create src");

        for (relative, contents) in files {
            let path = src_dir.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(&path, contents).expect("write source");
        }

        tmp
    }
}
