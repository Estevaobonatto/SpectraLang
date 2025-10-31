use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub span: Span,
    pub items: Vec<Item>,
}

impl Module {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Placeholder,
}
