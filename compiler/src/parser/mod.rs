use crate::{
    ast::Module,
    error::ParseError,
    span::Span,
    token::{Token, TokenKind},
};

pub struct Parser<'tokens> {
    tokens: &'tokens [Token],
    cursor: usize,
}

impl<'tokens> Parser<'tokens> {
    pub fn new(tokens: &'tokens [Token]) -> Self {
        Self { tokens, cursor: 0 }
    }

    pub fn parse(&mut self) -> Result<Module, Vec<ParseError>> {
        let name = "module::placeholder";
        let span = self
            .tokens
            .first()
            .map(|token| token.span)
            .unwrap_or_else(Span::dummy);
        Ok(Module::new(name, span))
    }

    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    pub fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.cursor);
        if token
            .map(|t| matches!(t.kind, TokenKind::EndOfFile))
            .unwrap_or(false)
        {
            return token;
        }
        self.cursor = self.cursor.saturating_add(1);
        token
    }
}
