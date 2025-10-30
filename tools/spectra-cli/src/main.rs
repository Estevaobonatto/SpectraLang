use std::{fs, path::PathBuf, process::exit};

use clap::Parser as ClapParser;
use spectra_compiler::{ast::{Item, Module}, lexer::Lexer, parser::Parser, semantic};

#[derive(ClapParser, Debug)]
#[command(name = "spectra", about = "SpectraLang CLI prototype", version)]
struct Cli {
    /// Source file to lex and parse
    #[arg(value_name = "FILE", required = true)]
    input: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    if let Err(code) = run(cli.input) {
        exit(code);
    }
}

fn run(paths: Vec<PathBuf>) -> Result<(), i32> {
    let mut modules = Vec::new();

    for path in &paths {
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

    for (path, module) in &modules {
        let module_name = module
            .name
            .as_ref()
            .map(|path| path.segments.join("."))
            .unwrap_or_else(|| "<anonymous>".to_string());
        let function_count = module
            .items
            .iter()
            .filter(|item| matches!(item, Item::Function(_)))
            .count();
        let item_count = module.items.len();
        let statement_count = item_count - function_count;
        println!(
            "Parsed module '{module_name}' with {item_count} item(s) ({function_count} function(s), {statement_count} statement(s)) from {}",
            path.display()
        );
    }

    Ok(())
}
