mod expression;
mod item;
mod module;
mod statement;
mod type_annotation;

use crate::{
    ast::Module,
    error::ParseError,
    span::Span,
    token::{Keyword, Token, TokenKind},
};

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
            errors: Vec::new(),
        }
    }

    pub fn parse(mut self) -> Result<Module, Vec<ParseError>> {
        let module = self.parse_module();

        if self.errors.is_empty() {
            Ok(module)
        } else {
            Err(self.errors)
        }
    }

    // === Token Navigation Methods ===

    fn current(&self) -> &Token {
        self.tokens
            .get(self.position)
            .unwrap_or_else(|| self.tokens.last().expect("tokens should not be empty"))
    }

    #[allow(dead_code)]
    fn peek(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.position + offset)
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.position += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current().kind, TokenKind::EndOfFile)
    }

    // === Token Checking Methods ===

    #[allow(dead_code)]
    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(&self.current().kind, TokenKind::Keyword(k) if *k == keyword)
    }

    fn check_symbol(&self, symbol: char) -> bool {
        matches!(&self.current().kind, TokenKind::Symbol(s) if *s == symbol)
    }

    #[allow(dead_code)]
    fn check_identifier(&self) -> bool {
        matches!(self.current().kind, TokenKind::Identifier(_))
    }

    // === Token Consumption Methods ===

    fn consume_keyword(&mut self, keyword: Keyword, error_message: &str) -> Result<Span, ()> {
        if self.check_keyword(keyword) {
            let span = self.current().span;
            self.advance();
            Ok(span)
        } else {
            self.error(error_message);
            Err(())
        }
    }

    fn consume_symbol(&mut self, symbol: char, error_message: &str) -> Result<Span, ()> {
        if self.check_symbol(symbol) {
            let span = self.current().span;
            self.advance();
            Ok(span)
        } else {
            self.error(error_message);
            Err(())
        }
    }

    fn consume_identifier(&mut self, error_message: &str) -> Result<(String, Span), ()> {
        if let TokenKind::Identifier(name) = &self.current().kind.clone() {
            let name = name.clone();
            let span = self.current().span;
            self.advance();
            Ok((name, span))
        } else {
            self.error(error_message);
            Err(())
        }
    }

    // === Error Handling ===

    fn error(&mut self, message: &str) {
        let span = self.current().span;
        self.errors.push(ParseError::new(message, span));
    }

    #[allow(dead_code)]
    fn error_at(&mut self, message: &str, span: Span) {
        self.errors.push(ParseError::new(message, span));
    }

    // === Synchronization ===

    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.check_symbol(';') {
                self.advance();
                return;
            }

            match &self.current().kind {
                TokenKind::Keyword(Keyword::Module)
                | TokenKind::Keyword(Keyword::Import)
                | TokenKind::Keyword(Keyword::Fn)
                | TokenKind::Keyword(Keyword::Class)
                | TokenKind::Keyword(Keyword::Trait)
                | TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::Return) => return,
                _ => {}
            }

            self.advance();
        }
    }
}
