use crate::{
    ast::{Import, Module, Visibility},
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
        self.parse_import_with_visibility(Visibility::Private)
    }

    pub(super) fn parse_import_with_visibility(
        &mut self,
        visibility: Visibility,
    ) -> Result<Import, ()> {
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

        let alias = if self.check_keyword(Keyword::As) {
            self.advance(); // consume 'as'
            let (alias, _) = self.consume_identifier("Expected alias name after 'as'")?;
            Some(alias)
        } else {
            None
        };

        let end_span = self.consume_symbol(';', "Expected ';' after import path")?;

        Ok(Import {
            path,
            alias,
            visibility,
            span: span_union(start_span, end_span),
        })
    }
}
