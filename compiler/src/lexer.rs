use crate::{
    error::{LexError, LexResult},
    span::{Location, Span},
    token::{Keyword, Token, TokenKind},
};

/// Lexical analyzer that turns raw source code into a stream of tokens.
pub struct Lexer<'a> {
    source: &'a [u8],
    len: usize,
    pos: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            len: source.len(),
            pos: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn tokenize(mut self) -> LexResult<Vec<Token>> {
        while !self.is_eof() {
            self.skip_whitespace_and_comments();
            if self.is_eof() {
                break;
            }

            let start_pos = self.pos;
            let start_loc = self.location();

            if let Some(token) = self.next_token(start_pos, start_loc) {
                self.tokens.push(token);
            }
        }

        let loc = self.location();
        let span = Span::new(self.pos, self.pos, loc, loc);
        self.tokens
            .push(Token::new(TokenKind::Eof, String::new(), span));

        if self.errors.is_empty() {
            Ok(self.tokens)
        } else {
            Err(self.errors)
        }
    }

    fn next_token(&mut self, start_pos: usize, start_loc: Location) -> Option<Token> {
        let ch = self.advance()?;
        let token = match ch {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.read_identifier(start_pos, start_loc),
            b'0'..=b'9' => self.read_number(start_pos, start_loc),
            b'"' => match self.read_string(start_pos, start_loc) {
                Ok(token) => token,
                Err(err) => {
                    self.errors.push(err);
                    return None;
                }
            },
            b':' => {
                if self.match_byte(b':') {
                    self.make_token(TokenKind::Scope, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Colon, start_pos, start_loc)
                }
            }
            b';' => self.make_token(TokenKind::Semicolon, start_pos, start_loc),
            b',' => self.make_token(TokenKind::Comma, start_pos, start_loc),
            b'.' => self.make_token(TokenKind::Dot, start_pos, start_loc),
            b'-' => {
                if self.match_byte(b'>') {
                    self.make_token(TokenKind::Arrow, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Minus, start_pos, start_loc)
                }
            }
            b'+' => self.make_token(TokenKind::Plus, start_pos, start_loc),
            b'*' => self.make_token(TokenKind::Star, start_pos, start_loc),
            b'/' => self.make_token(TokenKind::Slash, start_pos, start_loc),
            b'%' => self.make_token(TokenKind::Percent, start_pos, start_loc),
            b'{' => self.make_token(TokenKind::LBrace, start_pos, start_loc),
            b'}' => self.make_token(TokenKind::RBrace, start_pos, start_loc),
            b'(' => self.make_token(TokenKind::LParen, start_pos, start_loc),
            b')' => self.make_token(TokenKind::RParen, start_pos, start_loc),
            b'[' => self.make_token(TokenKind::LBracket, start_pos, start_loc),
            b']' => self.make_token(TokenKind::RBracket, start_pos, start_loc),
            b'=' => {
                if self.match_byte(b'>') {
                    self.make_token(TokenKind::Arrow, start_pos, start_loc)
                } else if self.match_byte(b'=') {
                    self.make_token(TokenKind::EqualEqual, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Equal, start_pos, start_loc)
                }
            }
            b'!' => {
                if self.match_byte(b'=') {
                    self.make_token(TokenKind::BangEqual, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Bang, start_pos, start_loc)
                }
            }
            b'<' => {
                if self.match_byte(b'=') {
                    self.make_token(TokenKind::LessEqual, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Less, start_pos, start_loc)
                }
            }
            b'>' => {
                if self.match_byte(b'=') {
                    self.make_token(TokenKind::GreaterEqual, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Greater, start_pos, start_loc)
                }
            }
            b'&' => {
                if self.match_byte(b'&') {
                    self.make_token(TokenKind::AmpersandAmpersand, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Ampersand, start_pos, start_loc)
                }
            }
            b'|' => {
                if self.match_byte(b'|') {
                    self.make_token(TokenKind::PipePipe, start_pos, start_loc)
                } else {
                    self.make_token(TokenKind::Pipe, start_pos, start_loc)
                }
            }
            byte => {
                let span = self.span_from(start_pos, start_loc);
                self.errors.push(LexError::new(
                    format!("unexpected character `{}`", byte as char),
                    span,
                ));
                return None;
            }
        };

        Some(token)
    }

    fn read_identifier(&mut self, start_pos: usize, start_loc: Location) -> Token {
        while matches!(
            self.peek(),
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
        ) {
            self.advance();
        }

        let span = self.span_from(start_pos, start_loc);
        let lexeme = self.slice(span.start, span.end);
        if let Some(keyword) = Keyword::from_identifier(&lexeme) {
            Token::new(TokenKind::Keyword(keyword), lexeme, span)
        } else {
            Token::new(TokenKind::Identifier, lexeme, span)
        }
    }

    fn read_number(&mut self, start_pos: usize, start_loc: Location) -> Token {
        while matches!(self.peek(), Some(b'0'..=b'9' | b'_')) {
            self.advance();
        }

        let mut kind = TokenKind::Integer;
        if self.peek() == Some(b'.') && matches!(self.peek_next(), Some(b'0'..=b'9')) {
            kind = TokenKind::Float;
            self.advance(); // consume '.'
            while matches!(self.peek(), Some(b'0'..=b'9' | b'_')) {
                self.advance();
            }
        }

        let span = self.span_from(start_pos, start_loc);
        let mut lexeme = self.slice(span.start, span.end);
        lexeme.retain(|c| c != '_');
        Token::new(kind, lexeme, span)
    }

    fn read_string(&mut self, start_pos: usize, start_loc: Location) -> Result<Token, LexError> {
        let mut buffer = Vec::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.advance();
                    break;
                }
                Some(b'\\') => {
                    self.advance();
                    let escape = match self.advance() {
                        Some(b'"') => b'"',
                        Some(b'\\') => b'\\',
                        Some(b'n') => b'\n',
                        Some(b'r') => b'\r',
                        Some(b't') => b'\t',
                        Some(other) => {
                            let span = self.span_from(start_pos, start_loc);
                            return Err(LexError::new(
                                format!("unsupported escape \\{}", other as char),
                                span,
                            ));
                        }
                        None => {
                            let span = self.span_from(start_pos, start_loc);
                            return Err(LexError::new("unterminated escape sequence", span));
                        }
                    };
                    buffer.push(escape);
                }
                Some(b'\n') | None => {
                    let span = self.span_from(start_pos, start_loc);
                    return Err(LexError::new("unterminated string literal", span));
                }
                Some(ch) => {
                    self.advance();
                    buffer.push(ch);
                }
            }
        }

        let span = self.span_from(start_pos, start_loc);
        let lexeme = String::from_utf8(buffer).expect("lexer emitted invalid UTF-8");
        Ok(Token::new(TokenKind::String, lexeme, span))
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r') => {
                    self.advance();
                }
                Some(b'\n') => {
                    self.advance();
                }
                Some(b'/') if self.peek_next() == Some(b'/') => {
                    self.advance();
                    self.advance();
                    while !matches!(self.peek(), None | Some(b'\n')) {
                        self.advance();
                    }
                }
                Some(b'/') if self.peek_next() == Some(b'*') => {
                    let start_pos = self.pos;
                    let start_loc = self.location();
                    self.advance();
                    self.advance();

                    let mut terminated = false;
                    while let Some(ch) = self.peek() {
                        if ch == b'*' && self.peek_next() == Some(b'/') {
                            self.advance();
                            self.advance();
                            terminated = true;
                            break;
                        }
                        self.advance();
                    }

                    if !terminated {
                        let span = Span::new(start_pos, self.pos, start_loc, self.location());
                        self.errors
                            .push(LexError::new("unterminated block comment", span));
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn match_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> Option<u8> {
        if self.pos >= self.len {
            return None;
        }
        let byte = self.source[self.pos];
        self.pos += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.source.get(self.pos + 1).copied()
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.len
    }

    fn location(&self) -> Location {
        Location::new(self.line, self.column)
    }

    fn slice(&self, start: usize, end: usize) -> String {
        std::str::from_utf8(&self.source[start..end])
            .expect("lexer should emit valid UTF-8 slices")
            .to_string()
    }

    fn span_from(&self, start: usize, start_loc: Location) -> Span {
        Span::new(start, self.pos, start_loc, self.location())
    }

    fn make_token(&mut self, kind: TokenKind, start: usize, start_loc: Location) -> Token {
        let span = self.span_from(start, start_loc);
        let lexeme = self.slice(span.start, span.end);
        Token::new(kind, lexeme, span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<TokenKind> {
        Lexer::new(source)
            .tokenize()
            .expect("lexing should succeed")
            .into_iter()
            .map(|tok| tok.kind)
            .collect()
    }

    #[test]
    fn lex_basic_keywords_and_identifiers() {
        let kinds = lex("module core { let value = 42; }");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword(Keyword::Module),
                TokenKind::Identifier,
                TokenKind::LBrace,
                TokenKind::Keyword(Keyword::Let),
                TokenKind::Identifier,
                TokenKind::Equal,
                TokenKind::Integer,
                TokenKind::Semicolon,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_numbers_and_operators() {
        let kinds = lex("let x = 10 + 20.5;");
        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword(Keyword::Let),
                TokenKind::Identifier,
                TokenKind::Equal,
                TokenKind::Integer,
                TokenKind::Plus,
                TokenKind::Float,
                TokenKind::Semicolon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_string_literal() {
        let tokens = Lexer::new("let s = \"hello\";")
            .tokenize()
            .expect("no errors");
        assert_eq!(tokens[3].lexeme, "hello");
        assert_eq!(tokens[3].kind, TokenKind::String);
    }

    #[test]
    fn report_unterminated_string() {
        let result = Lexer::new("let s = \"oops").tokenize();
        assert!(result.is_err());
        let errors = result.err().unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("unterminated string"));
    }

    #[test]
    fn report_unterminated_block_comment() {
        let result = Lexer::new("/* comment").tokenize();
        assert!(result.is_err());
        let errors = result.err().unwrap();
        assert!(errors
            .iter()
            .any(|error| error.message.contains("unterminated block comment")));
    }
}
