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

    pub(super) fn parse_import(&mut self, is_reexport: bool) -> Result<Import, ()> {
        // Supports three forms:
        //   import path.to.module;
        //   import path.to.module as alias;
        //   import { name1, name2 } from path.to.module;
        //   pub import ...   (re-export: exposes imported symbols to callers)
        let start_span = self.consume_keyword(Keyword::Import, "Expected 'import' keyword")?;

        // Check for named-import form: import { ... } from path;
        if self.check_symbol('{') {
            self.advance(); // consume '{'
            let mut names = Vec::new();
            loop {
                let (name, _) = self.consume_identifier("Expected import name")?;
                names.push(name);
                if self.check_symbol(',') {
                    self.advance(); // consume ','
                    if self.check_symbol('}') {
                        break; // trailing comma
                    }
                } else {
                    break;
                }
            }
            self.consume_symbol('}', "Expected '}' after import names")?;

            // Expect 'from' identifier
            match &self.current().kind {
                crate::token::TokenKind::Identifier(kw) if kw == "from" => {
                    self.advance(); // consume 'from'
                }
                _ => {
                    self.error_at("Expected 'from' after import names", self.current().span);
                    return Err(());
                }
            }

            let mut path = Vec::new();
            let (first, _) = self.consume_identifier("Expected module path")?;
            path.push(first);
            while self.check_symbol('.') {
                self.advance();
                let (seg, _) = self.consume_identifier("Expected identifier after '.'")?;
                path.push(seg);
            }
            let end_span = self.consume_symbol(';', "Expected ';' after import path")?;

            return Ok(Import {
                path,
                alias: None,
                names: Some(names),
                is_reexport,
                span: span_union(start_span, end_span),
            });
        }

        // Standard path form
        let mut path = Vec::new();
        let (name, _) = self.consume_identifier("Expected module path")?;
        path.push(name);
        while self.check_symbol('.') {
            self.advance();
            let (seg, _) = self.consume_identifier("Expected identifier after '.'")?;
            path.push(seg);
        }

        // Optional alias: import path as alias;
        let alias = match &self.current().kind {
            crate::token::TokenKind::Identifier(kw) if kw == "as" => {
                self.advance(); // consume 'as'
                let (alias_name, _) = self.consume_identifier("Expected alias name after 'as'")?;
                Some(alias_name)
            }
            _ => None,
        };

        let end_span = self.consume_symbol(';', "Expected ';' after import")?;

        Ok(Import {
            path,
            alias,
            names: None,
            is_reexport,
            span: span_union(start_span, end_span),
        })
    }
}
