mod compiler_integration;

use compiler_integration::SpectraCompiler;
use spectra_compiler::CompilationOptions;
use std::{env, fs, process};

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

    // Compile each file
    let mut compiler = SpectraCompiler::new(options);
    let mut has_failures = false;

    for path in &file_paths {
        println!("\n📦 Compiling: {}", path);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        match fs::read_to_string(path) {
            Ok(source) => {
                match compiler.compile(&source, path) {
                    Ok(()) => {
                        println!("\n✅ Successfully compiled: {}", path);
                    }
                    Err(error) => {
                        has_failures = true;
                        eprintln!("\n❌ Compilation failed: {}", path);
                        eprintln!("{}", error);
                    }
                }
            }
            Err(error) => {
                has_failures = true;
                eprintln!("\n❌ Failed to read file: {}", path);
                eprintln!("Error: {}", error);
            }
        }
    }

    if has_failures {
        eprintln!("\n💥 Compilation failed with errors");
        process::exit(1);
    } else {
        println!("\n🎉 All files compiled successfully!");
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
    println!("    --no-optimize, -O0 Disable all optimizations");
    println!("    -O1                Enable basic optimizations");
    println!("    -O2                Enable moderate optimizations (default)");
    println!("    -O3                Enable aggressive optimizations");
    println!();
    println!("EXAMPLES:");
    println!("    spectra program.spectra");
    println!("    spectra --dump-ir -O3 program.spectra");
    println!("    spectra file1.spectra file2.spectra");
}
