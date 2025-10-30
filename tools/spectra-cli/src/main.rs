use std::{fs, path::PathBuf, process::exit};

use clap::Parser as ClapParser;
use spectra_compiler::{lexer::Lexer, parser::Parser};

#[derive(ClapParser, Debug)]
#[command(name = "spectra", about = "SpectraLang CLI prototype", version)]
struct Cli {
    /// Source file to lex and parse
    #[arg(value_name = "FILE")]
    input: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    if let Err(code) = run(cli.input) {
        exit(code);
    }
}

fn run(path: PathBuf) -> Result<(), i32> {
    let source = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("error: failed to read '{}': {}", path.display(), err);
            return Err(2);
        }
    };

    match Lexer::new(&source).tokenize() {
        Ok(tokens) => match Parser::new(&tokens).parse() {
            Ok(module) => {
                println!(
                    "Parsed {} top-level declaration(s) from {}",
                    module.declarations.len(),
                    path.display()
                );
                Ok(())
            }
            Err(errors) => {
                eprintln!("parse error(s):");
                for error in errors {
                    eprintln!("  - {}", error);
                }
                Err(3)
            }
        },
        Err(errors) => {
            eprintln!("lexical error(s):");
            for error in errors {
                eprintln!("  - {}", error);
            }
            Err(3)
        }
    }
}
