use crate::{
    ast::{Function, Item, Module, TypeName},
    span::Span,
};
/// Represents the entry point discovered when scanning SpectraLang modules.
#[derive(Debug)]
pub struct EntryPoint<'m> {
    pub module: &'m Module,
    pub function: &'m Function,
}

/// Error returned when no suitable entry point can be determined.
#[derive(Debug, Clone)]
pub struct EntryPointError {
    pub message: String,
    pub span: Option<Span>,
}

impl EntryPointError {
    pub fn new(message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

/// Searches the provided modules for `fn main(): i32` definitions sem parâmetros.
/// Caso `preferred_main` seja informado, o módulo correspondente é selecionado.
/// Quando não informado, exige um único candidato válido.
pub fn find_console_entry_point<'m>(
    modules: &[&'m Module],
    preferred_main: Option<&str>,
) -> Result<EntryPoint<'m>, Vec<EntryPointError>> {
    let mut candidates: Vec<EntryPoint<'m>> = Vec::new();
    let mut errors: Vec<EntryPointError> = Vec::new();

    for module in modules {
        for item in &module.items {
            let Item::Function(function) = item else {
                continue;
            };

            if function.name != "main" {
                continue;
            }

            if !function.parameters.is_empty() {
                errors.push(EntryPointError::new(
                    "`main` must not accept parameters",
                    Some(function.span),
                ));
            }

            match &function.return_type {
                Some(ty) if is_i32(ty) => {}
                Some(ty) => errors.push(EntryPointError::new(
                    format!(
                        "`main` must return i32 but returns {}",
                        ty.segments.join("::")
                    ),
                    Some(ty.span),
                )),
                None => errors.push(EntryPointError::new(
                    "`main` must declare a return type of i32",
                    Some(function.span),
                )),
            }

            candidates.push(EntryPoint { module, function });
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    if let Some(target) = preferred_main {
        if let Some(entry) = candidates.iter().find(|candidate| {
            module_qualified_name(candidate.module)
                .as_deref()
                .map(|name| name == target)
                .unwrap_or(false)
        }) {
            return Ok(EntryPoint {
                module: entry.module,
                function: entry.function,
            });
        }

        return Err(vec![EntryPointError::new(
            format!(
                "no entry point `fn main` found in module '{}' (available: {})",
                target,
                describe_candidate_modules(&candidates)
            ),
            None,
        )]);
    }

    match candidates.len() {
        0 => Err(vec![EntryPointError::new(
            "no entry point found; define `fn main(): i32 { ... }`",
            None,
        )]),
        1 => Ok(candidates.remove(0)),
        count => {
            let mut conflicts: Vec<EntryPointError> = Vec::new();
            for entry in candidates {
                conflicts.push(EntryPointError::new(
                    format!(
                        "multiple entry points detected ({} total); `main` defined in module '{}' — selecione com `--main <module>`",
                        count,
                        module_qualified_name(entry.module)
                            .unwrap_or_else(|| "<anonymous>".to_string())
                    ),
                    Some(entry.function.span),
                ));
            }
            Err(conflicts)
        }
    }
}

fn is_i32(ty: &TypeName) -> bool {
    ty.segments.len() == 1 && ty.segments[0] == "i32"
}

fn module_qualified_name(module: &Module) -> Option<String> {
    module.name.as_ref().map(|path| path.segments.join("."))
}

fn describe_candidate_modules(entries: &[EntryPoint<'_>]) -> String {
    if entries.is_empty() {
        return "<nenhum>".to_string();
    }

    let mut names: Vec<String> = entries
        .iter()
        .map(|entry| {
            module_qualified_name(entry.module).unwrap_or_else(|| "<anonymous>".to_string())
        })
        .collect();
    names.sort();
    names.dedup();
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::Lexer, parser::Parser};

    fn module_from_source(source: &str) -> Module {
        let tokens = Lexer::new(source).tokenize().expect("tokenize ok");
        Parser::new(&tokens).parse().expect("parse ok")
    }

    #[test]
    fn detects_valid_entry_point() {
        let module = module_from_source("fn main(): i32 { return 0; }");
        let modules = vec![&module];
        let entry = find_console_entry_point(&modules, None).expect("entry point ok");
        assert_eq!(entry.function.name, "main");
    }

    #[test]
    fn requires_main_presence() {
        let module = module_from_source("fn helper(): i32 { return 0; }");
        let modules = vec![&module];
        let errors = find_console_entry_point(&modules, None).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("no entry point"));
    }

    #[test]
    fn rejects_parameters() {
        let module = module_from_source("fn main(x: i32): i32 { return x; }");
        let modules = vec![&module];
        let errors = find_console_entry_point(&modules, None).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("must not accept parameters")));
    }

    #[test]
    fn enforces_return_type() {
        let module = module_from_source("fn main() { return; }");
        let modules = vec![&module];
        let errors = find_console_entry_point(&modules, None).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("must declare a return type")));
    }

    #[test]
    fn rejects_multiple_mains() {
        let module_a = module_from_source("module app.alpha; fn main(): i32 { return 0; }");
        let module_b = module_from_source("module app.beta; fn main(): i32 { return 1; }");
        let modules = vec![&module_a, &module_b];
        let errors = find_console_entry_point(&modules, None).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("multiple entry points")));
    }

    #[test]
    fn selects_preferred_entry_point() {
        let module_a = module_from_source("module app.alpha; fn main(): i32 { return 0; }");
        let module_b = module_from_source("module app.beta; fn main(): i32 { return 1; }");
        let modules = vec![&module_a, &module_b];

        let entry = find_console_entry_point(&modules, Some("app.beta")).expect("selected entry");
        assert_eq!(
            module_qualified_name(entry.module).as_deref(),
            Some("app.beta")
        );
    }
}
