use crate::{
    ast::{Import, Item, Module, Visibility},
    span::{span_union, Span},
    token::{Keyword, TokenKind},
};

use super::Parser;

impl Parser {
    pub(super) fn parse_module(&mut self) -> Module {
        let mut disable_prelude = false;

        while self.check_symbol('#') {
            disable_prelude |= self.parse_module_attribute();
        }

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
        module.disable_prelude = disable_prelude;

        // Parse module items
        while !self.is_at_end() {
            match self.parse_item() {
                Ok(item) => module.items.push(item),
                Err(_) => self.synchronize(),
            }
        }

        self.inject_prelude_import(&mut module);

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
            synthetic: false,
        })
    }
}

impl Parser {
    fn parse_module_attribute(&mut self) -> bool {
        let mut disable_prelude = false;

        // Consume '#'
        self.advance();

        if self
            .consume_symbol('!', "Expected '!' after '#' in module attribute")
            .is_err()
        {
            return disable_prelude;
        }

        if self
            .consume_symbol('[', "Expected '[' to begin module attribute")
            .is_err()
        {
            return disable_prelude;
        }

        let attr_name = match &self.current().kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => {
                self.error("Expected attribute name");
                return disable_prelude;
            }
        };

        if self
            .consume_symbol(']', "Expected ']' to close module attribute")
            .is_err()
        {
            return disable_prelude;
        }

        match attr_name.as_str() {
            "no_prelude" => disable_prelude = true,
            _ => {
                self.error("Unknown module attribute");
            }
        }

        disable_prelude
    }

    fn inject_prelude_import(&self, module: &mut Module) {
        if module.disable_prelude || is_builtin_module(&module.name) {
            return;
        }

        if module
            .items
            .iter()
            .any(|item| matches!(item, Item::Import(import) if is_prelude_path(&import.path)))
        {
            return;
        }

        let import = Import {
            path: vec!["std".to_string(), "prelude".to_string()],
            alias: None,
            visibility: Visibility::Private,
            span: Span::dummy(),
            synthetic: true,
        };

        module.items.insert(0, Item::Import(import));
    }
}

fn is_builtin_module(name: &str) -> bool {
    name == "std"
        || name.starts_with("std.")
        || name == "spectra.std"
        || name.starts_with("spectra.std.")
        || is_prelude_module(name)
}

fn is_prelude_module(name: &str) -> bool {
    name == "std.prelude" || name == "spectra.std.prelude"
}

fn is_prelude_path(path: &[String]) -> bool {
    matches!(path, [first, second] if first == "std" && second == "prelude")
}
