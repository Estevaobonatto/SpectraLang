use crate::{
    error::LexError,
    span::{Location, Span},
    token::{Keyword, Token, TokenKind},
};

pub struct Lexer<'source> {
    source: &'source str,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Self { source }
    }

    pub fn tokenize(&self) -> Result<Vec<Token>, Vec<LexError>> {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();

        let characters: Vec<(usize, char)> = self.source.char_indices().collect();
        let mut index = 0;
        let length = characters.len();
        let mut line = 1;
        let mut column = 1;

        while index < length {
            let (offset, ch) = characters[index];
            let start_location = Location::new(line, column);

            match ch {
                ' ' | '\t' | '\r' => {
                    bump_position(ch, &mut line, &mut column);
                    index += 1;
                }
                '\n' => {
                    bump_position(ch, &mut line, &mut column);
                    index += 1;
                }
                '/' if index + 1 < length && characters[index + 1].1 == '/' => {
                    // Consume comment start
                    bump_position('/', &mut line, &mut column);
                    index += 1;
                    bump_position('/', &mut line, &mut column);
                    index += 1;

                    while index < length {
                        let (_, comment_char) = characters[index];
                        if comment_char == '\n' {
                            break;
                        }
                        bump_position(comment_char, &mut line, &mut column);
                        index += 1;
                    }
                }
                ch if is_identifier_start(ch) => {
                    bump_position(ch, &mut line, &mut column);
                    let mut end_index = index + 1;
                    while end_index < length {
                        let (_, next_char) = characters[end_index];
                        if is_identifier_continue(next_char) {
                            bump_position(next_char, &mut line, &mut column);
                            end_index += 1;
                        } else {
                            break;
                        }
                    }

                    let end_offset = if end_index < length {
                        characters[end_index].0
                    } else {
                        self.source.len()
                    };
                    let end_location = Location::new(line, column);
                    let text = &self.source[offset..end_offset];
                    let token_kind = Keyword::from_identifier(text)
                        .map(TokenKind::Keyword)
                        .unwrap_or_else(|| TokenKind::Identifier(text.to_string()));
                    tokens.push(Token::new(
                        token_kind,
                        Span::new(offset, end_offset, start_location, end_location),
                    ));
                    index = end_index;
                }
                ch if ch.is_ascii_digit() => {
                    bump_position(ch, &mut line, &mut column);
                    let mut end_index = index + 1;
                    let mut seen_dot = false;

                    while end_index < length {
                        let (_, next_char) = characters[end_index];
                        if next_char.is_ascii_digit() {
                            bump_position(next_char, &mut line, &mut column);
                            end_index += 1;
                        } else if next_char == '.' && !seen_dot {
                            if end_index + 1 < length
                                && characters[end_index + 1].1.is_ascii_digit()
                            {
                                seen_dot = true;
                                bump_position(next_char, &mut line, &mut column);
                                end_index += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    let end_offset = if end_index < length {
                        characters[end_index].0
                    } else {
                        self.source.len()
                    };
                    let end_location = Location::new(line, column);
                    let text = &self.source[offset..end_offset];
                    tokens.push(Token::new(
                        TokenKind::Number(text.to_string()),
                        Span::new(offset, end_offset, start_location, end_location),
                    ));
                    index = end_index;
                }
                '"' => {
                    if let Some(relative) = self.source[offset + 1..].find('"') {
                        let closing_offset = offset + 1 + relative;
                        let end_offset = closing_offset + 1;

                        while index < length {
                            let (current_offset, current_char) = characters[index];
                            if current_offset >= end_offset {
                                break;
                            }
                            bump_position(current_char, &mut line, &mut column);
                            index += 1;
                        }

                        let end_location = Location::new(line, column);
                        let value = self.source[offset + 1..closing_offset].to_string();
                        tokens.push(Token::new(
                            TokenKind::StringLiteral(value),
                            Span::new(offset, end_offset, start_location, end_location),
                        ));
                    } else {
                        while index < length {
                            let (_, current_char) = characters[index];
                            bump_position(current_char, &mut line, &mut column);
                            index += 1;
                        }
                        let end_location = Location::new(line, column);
                        errors.push(LexError::new(
                            "unterminated string literal",
                            Span::new(offset, self.source.len(), start_location, end_location),
                        ));
                    }
                }
                ch if is_symbol_char(ch) => {
                    // Check for two-character operators
                    let next_char = if index + 1 < length {
                        Some(characters[index + 1].1)
                    } else {
                        None
                    };

                    let (token_kind, chars_consumed) = match (ch, next_char) {
                        ('=', Some('=')) => {
                            (TokenKind::Operator(crate::token::Operator::EqualEqual), 2)
                        }
                        ('!', Some('=')) => {
                            (TokenKind::Operator(crate::token::Operator::NotEqual), 2)
                        }
                        ('<', Some('=')) => {
                            (TokenKind::Operator(crate::token::Operator::LessEqual), 2)
                        }
                        ('>', Some('=')) => {
                            (TokenKind::Operator(crate::token::Operator::GreaterEqual), 2)
                        }
                        ('&', Some('&')) => (TokenKind::Operator(crate::token::Operator::And), 2),
                        ('|', Some('|')) => (TokenKind::Operator(crate::token::Operator::Or), 2),
                        ('-', Some('>')) => (TokenKind::Operator(crate::token::Operator::Arrow), 2),
                        _ => (TokenKind::Symbol(ch), 1),
                    };

                    for _ in 0..chars_consumed {
                        let (_, current_char) = characters[index];
                        bump_position(current_char, &mut line, &mut column);
                        index += 1;
                    }

                    let end_offset = if index < length {
                        characters[index].0
                    } else {
                        self.source.len()
                    };
                    let end_location = Location::new(line, column);
                    tokens.push(Token::new(
                        token_kind,
                        Span::new(offset, end_offset, start_location, end_location),
                    ));
                }
                _ => {
                    bump_position(ch, &mut line, &mut column);
                    let end_offset = offset + ch.len_utf8();
                    let end_location = Location::new(line, column);
                    errors.push(LexError::new(
                        format!("unexpected character `{}`", ch),
                        Span::new(offset, end_offset, start_location, end_location),
                    ));
                    index += 1;
                }
            }
        }

        let eof_span = Span::new(
            self.source.len(),
            self.source.len(),
            Location::new(line, column),
            Location::new(line, column),
        );
        tokens.push(Token::new(TokenKind::EndOfFile, eof_span));

        if errors.is_empty() {
            Ok(tokens)
        } else {
            Err(errors)
        }
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

fn is_symbol_char(ch: char) -> bool {
    matches!(
        ch,
        '(' | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | ','
            | ';'
            | ':'
            | '.'
            | '='
            | '+'
            | '-'
            | '*'
            | '/'
            | '%'
            | '@'
            | '<'
            | '>'
            | '!'
            | '&'
            | '|'
    )
}

fn bump_position(ch: char, line: &mut usize, column: &mut usize) {
    if ch == '\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_basic_tokens() {
        let source = "module app.core; fn main() { return; }";
        let tokens = Lexer::new(source).tokenize().expect("lexer should succeed");
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Keyword(Keyword::Module))));
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Identifier(ref ident) if ident == "app")));
        assert!(tokens.iter().any(
            |token| matches!(token.kind, TokenKind::Identifier(ref ident) if ident == "main")
        ));
    }
}
