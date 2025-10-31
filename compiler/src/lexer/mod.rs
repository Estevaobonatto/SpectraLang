use crate::{
    error::LexError,
    span::Span,
    token::{Token, TokenKind},
};

pub struct Lexer<'source> {
    source: &'source str,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Self { source }
    }

    pub fn tokenize(&self) -> Result<Vec<Token>, Vec<LexError>> {
        let _ = self.source;
        Ok(vec![Token::new(TokenKind::EndOfFile, Span::dummy())])
    }
}
