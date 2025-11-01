use crate::{
    ast::{Block, Function, FunctionParam, Item, Visibility},
    span::span_union,
    token::Keyword,
};

use super::Parser;

impl Parser {
    pub(super) fn parse_item(&mut self) -> Result<Item, ()> {
        match &self.current().kind {
            crate::token::TokenKind::Keyword(Keyword::Import) => {
                let import = self.parse_import()?;
                Ok(Item::Import(import))
            }
            crate::token::TokenKind::Keyword(Keyword::Pub) => {
                self.advance(); // consume 'pub'
                self.parse_item_with_visibility(Visibility::Public)
            }
            crate::token::TokenKind::Keyword(Keyword::Fn) => {
                self.parse_item_with_visibility(Visibility::Private)
            }
            _ => {
                self.error("Expected item declaration (import, fn, etc.)");
                Err(())
            }
        }
    }

    fn parse_item_with_visibility(&mut self, visibility: Visibility) -> Result<Item, ()> {
        match &self.current().kind {
            crate::token::TokenKind::Keyword(Keyword::Fn) => {
                let function = self.parse_function(visibility)?;
                Ok(Item::Function(function))
            }
            _ => {
                self.error("Expected function declaration");
                Err(())
            }
        }
    }

    pub(super) fn parse_function(&mut self, visibility: Visibility) -> Result<Function, ()> {
        // Expect: fn <name>(<params>) [-> type] { <body> }
        let start_span = self.consume_keyword(Keyword::Fn, "Expected 'fn' keyword")?;

        let (name, _name_span) = self.consume_identifier("Expected function name")?;

        self.consume_symbol('(', "Expected '(' after function name")?;

        let params = self.parse_function_params()?;

        self.consume_symbol(')', "Expected ')' after function parameters")?;

        // Optional return type
        let return_type = if matches!(&self.current().kind, crate::token::TokenKind::Operator(crate::token::Operator::Arrow)) {
            self.advance(); // consume '->'
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(Function {
            name,
            span: span_union(start_span, end_span),
            visibility,
            params,
            return_type,
            body,
        })
    }

    fn parse_function_params(&mut self) -> Result<Vec<FunctionParam>, ()> {
        let mut params = Vec::new();

        // Check for empty parameter list
        if self.check_symbol(')') {
            return Ok(params);
        }

        loop {
            let (name, name_span) = self.consume_identifier("Expected parameter name")?;

            // Optional type annotation
            let ty = if self.check_symbol(':') {
                self.advance(); // consume ':'
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            params.push(FunctionParam {
                name,
                span: name_span,
                ty,
            });

            if !self.check_symbol(',') {
                break;
            }
            self.advance(); // consume ','
        }

        Ok(params)
    }

    pub(super) fn parse_block(&mut self) -> Result<Block, ()> {
        let start_span = self.consume_symbol('{', "Expected '{' to start block")?;

        let mut statements = Vec::new();

        while !self.check_symbol('}') && !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(_) => self.synchronize(),
            }
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end block")?;

        Ok(Block {
            span: span_union(start_span, end_span),
            statements,
        })
    }
}
