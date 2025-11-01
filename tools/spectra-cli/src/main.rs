use spectra_compiler::{analyze_modules, Lexer, Module, Parser, Span};
use std::{env, fs, path::Path, process};

fn main() {
    let mut args = env::args().skip(1);
    let mut has_failures = false;
    let Some(first) = args.next() else {
        eprintln!("usage: spectra <source-files>...");
        process::exit(1);
    };

    let mut file_paths = vec![first];
    file_paths.extend(args);

    let mut parsed_modules: Vec<Module> = Vec::new();

    for path in &file_paths {
        match fs::read_to_string(path) {
            Ok(source) => {
                let lexer = Lexer::new(&source);
                let tokens = match lexer.tokenize() {
                    Ok(tokens) => tokens,
                    Err(errors) => {
                        has_failures = true;
                        for error in errors {
                            report_error(path, &error.message, error.span);
                        }
                        continue;
                    }
                };

                let parser = Parser::new(tokens);
                match parser.parse() {
                    Ok(module) => parsed_modules.push(module),
                    Err(errors) => {
                        has_failures = true;
                        for error in errors {
                            report_error(path, &error.message, error.span);
                        }
                    }
                }
            }
            Err(error) => {
                has_failures = true;
                report_error(
                    path,
                    &format!("failed to read file: {error}"),
                    Span::dummy(),
                );
            }
        }
    }

    if !parsed_modules.is_empty() {
        let module_refs: Vec<&Module> = parsed_modules.iter().collect();
        match analyze_modules(&module_refs) {
            Ok(()) => {}
            Err(errors) => {
                has_failures = true;
                for (index, error) in errors.iter().enumerate() {
                    let file = file_paths
                        .get(index)
                        .map(String::as_str)
                        .unwrap_or("<unknown>");
                    report_error(file, &error.message, error.span);
                }
            }
        }
    }

    if has_failures {
        process::exit(1);
    }
}

fn report_error(file: &str, message: &str, span: Span) {
    let display_path = Path::new(file)
        .canonicalize()
        .ok()
        .and_then(|p| p.to_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| file.to_string());
    eprintln!(
        "{}:{}:{}: {}",
        display_path, span.start_location.line, span.start_location.column, message
    );
}
