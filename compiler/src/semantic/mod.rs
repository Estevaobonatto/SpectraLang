use crate::{ast::Module, error::SemanticError};

pub fn analyze_modules(modules: &[&Module]) -> Result<(), Vec<SemanticError>> {
    let _ = modules;
    Ok(())
}
