use crate::{
    ast::{Import, Module},
    span::span_union,
    token::Keyword,
};

use super::Parser;

impl Parser {
    pub(super) fn parse_module(&mut self) -> Module {
        // Expect: module <name>;
        let start_span = match self.consume_keyword(Keyword::Module, "Expected 'module' keyword") {
            Ok(span) => span,
            Err(_) => {
                self.synchronize();
                return Module::new("error", self.current().span);
            }
        };

        let name = match self.consume_identifier("Expected module name") {
            Ok((name, _)) => name,
            Err(_) => {
                self.synchronize();
                return Module::new("error", start_span);
            }
        };

        let end_span = match self.consume_symbol(';', "Expected ';' after module name") {
            Ok(span) => span,
            Err(_) => {
                self.synchronize();
                return Module::new(name, start_span);
            }
        };

        let mut module = Module::new(name, span_union(start_span, end_span));

        // Parse module items
        while !self.is_at_end() {
            match self.parse_item() {
                Ok(item) => module.items.push(item),
                Err(_) => self.synchronize(),
            }
        }

        module
    }

    pub(super) fn parse_import(&mut self) -> Result<Import, ()> {
        // Expect: import path.to.module;
        let start_span = self.consume_keyword(Keyword::Import, "Expected 'import' keyword")?;

        let mut path = Vec::new();

        // First identifier
        let (name, _) = self.consume_identifier("Expected module path")?;
        path.push(name);

        // Additional path segments
        while self.check_symbol('.') {
            self.advance(); // consume '.'
            let (name, _) = self.consume_identifier("Expected identifier after '.'")?;
            path.push(name);
        }

        let end_span = self.consume_symbol(';', "Expected ';' after import path")?;

        Ok(Import {
            path,
            span: span_union(start_span, end_span),
        })
    }
}
