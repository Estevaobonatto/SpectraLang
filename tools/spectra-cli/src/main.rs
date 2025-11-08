mod compiler_integration;
mod project;

use compiler_integration::SpectraCompiler;
use spectra_compiler::CompilationOptions;
use std::{env, fs, path::PathBuf, process};

use project::ProjectPlan;

const KNOWN_EXPERIMENTAL_FEATURES: &[&str] = &["switch", "unless", "do-while", "loop"];

fn main() {
    let mut args = env::args().skip(1);

    // Parse command line arguments
    let mut file_paths = Vec::new();
    let mut options = CompilationOptions::default();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                return;
            }
            "--dump-ast" => {
                options.dump_ast = true;
            }
            "--dump-ir" => {
                options.dump_ir = true;
            }
            "--timings" | "-T" => {
                options.collect_metrics = true;
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
                options.run_jit = true;
            }
            "--enable-experimental" => {
                if let Some(feature) = args.next() {
                    options.experimental_features.insert(feature);
                } else {
                    eprintln!("Missing feature name after --enable-experimental");
                    process::exit(1);
                }
            }
            "--list-experimental" => {
                print_experimental_features();
                return;
            }
            arg if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                eprintln!("Use --help for usage information");
                process::exit(1);
            }
            _ => {
                file_paths.push(arg);
            }
        }
    }

    if file_paths.is_empty() {
        eprintln!("usage: spectra [options] <source-files>...");
        eprintln!("Use --help for more information");
        process::exit(1);
    }

    let entry_paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    let plan = match ProjectPlan::build(entry_paths) {
        Ok(plan) if !plan.modules().is_empty() => plan,
        Ok(_) => {
            eprintln!("No Spectra source files found to compile");
            process::exit(1);
        }
        Err(error) => {
            eprintln!("{}", error);
            process::exit(1);
        }
    };

    // Compile each module in dependency order
    let mut compiler = SpectraCompiler::new(options);
    let mut has_failures = false;

    for module in plan.modules() {
        println!(
            "\nCompiling module: {} ({})",
            module.name,
            module.path.display()
        );

        let filename = module.path.to_string_lossy().to_string();
        match fs::read_to_string(&module.path) {
            Ok(source) => match compiler.compile(&source, &filename) {
                Ok(()) => {
                    println!("\nSuccessfully compiled module '{}'", module.name);
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
        eprintln!("\n💥 Compilation failed with errors");
        process::exit(1);
    } else {
        println!("\nAll files compiled successfully!");
    }
}

fn print_help() {
    println!("SpectraLang Compiler");
    println!();
    println!("USAGE:");
    println!("    spectra [OPTIONS] <source-files>...");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help         Print this help message");
    println!("    --dump-ast         Print the AST for debugging");
    println!("    --dump-ir          Print the IR for debugging");
    println!("    --timings, -T      Report compilation and execution timings");
    println!("    --no-optimize, -O0 Disable all optimizations");
    println!("    -O1                Enable basic optimizations");
    println!("    -O2                Enable moderate optimizations (default)");
    println!("    -O3                Enable aggressive optimizations");
    println!("    --run, -r          Execute the program with the JIT after compiling");
    println!("    --enable-experimental <feature>");
    println!("                        Enable experimental language feature (may be repeated)");
    println!("    --list-experimental Show the list of experimental feature flags and exit");
    println!();
    println!("EXAMPLES:");
    println!("    spectra program.spectra");
    println!("    spectra --dump-ir -O3 program.spectra");
    println!("    spectra file1.spectra file2.spectra");
    println!();
    print_experimental_features();
}

fn print_experimental_features() {
    println!("Experimental features you can enable with --enable-experimental <feature>:");
    for feature in KNOWN_EXPERIMENTAL_FEATURES {
        println!("    - {}", feature);
    }
}
