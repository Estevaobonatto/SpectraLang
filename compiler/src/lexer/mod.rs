use crate::{
    error::LexError,
    span::{Location, Span},
    token::{Keyword, Operator, Token, TokenKind},
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
                    // Consume line comment start
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
                '/' if index + 1 < length && characters[index + 1].1 == '*' => {
                    // Consume block comment /* ... */
                    bump_position('/', &mut line, &mut column);
                    index += 1;
                    bump_position('*', &mut line, &mut column);
                    index += 1;

                    let mut closed = false;
                    while index < length {
                        let (_, comment_char) = characters[index];
                        if comment_char == '*'
                            && index + 1 < length
                            && characters[index + 1].1 == '/'
                        {
                            bump_position('*', &mut line, &mut column);
                            index += 1;
                            bump_position('/', &mut line, &mut column);
                            index += 1;
                            closed = true;
                            break;
                        }
                        bump_position(comment_char, &mut line, &mut column);
                        index += 1;
                    }

                    if !closed {
                        let end_location = Location::new(line, column);
                        errors.push(
                            LexError::new(
                                "unterminated block comment",
                                Span::new(
                                    offset,
                                    self.source.len(),
                                    start_location,
                                    end_location,
                                ),
                            )
                            .with_hint("Close the block comment with `*/`."),
                        );
                    }
                }
                ch if is_identifier_start(ch) => {
                    // Special case: f"..." is an f-string literal, not an identifier
                    if ch == 'f'
                        && index + 1 < length
                        && characters[index + 1].1 == '"'
                    {
                        // Consume 'f'
                        bump_position('f', &mut line, &mut column);
                        index += 1;
                        // Consume '"'
                        bump_position('"', &mut line, &mut column);
                        index += 1;

                        // Scan f-string content, preserving {expr} parts as-is
                        let mut raw_content = String::new();
                        let mut terminated = false;

                        while index < length {
                            let (_, sc) = characters[index];
                            if sc == '\\' && index + 1 < length {
                                let (_, escaped) = characters[index + 1];
                                bump_position('\\', &mut line, &mut column);
                                bump_position(escaped, &mut line, &mut column);
                                match escaped {
                                    '"'  => raw_content.push('"'),
                                    '\\' => raw_content.push('\\'),
                                    'n'  => raw_content.push('\n'),
                                    't'  => raw_content.push('\t'),
                                    'r'  => raw_content.push('\r'),
                                    '0'  => raw_content.push('\0'),
                                    other => {
                                        raw_content.push('\\');
                                        raw_content.push(other);
                                    }
                                }
                                index += 2;
                            } else if sc == '"' {
                                bump_position('"', &mut line, &mut column);
                                index += 1;
                                terminated = true;
                                break;
                            } else {
                                bump_position(sc, &mut line, &mut column);
                                raw_content.push(sc);
                                index += 1;
                            }
                        }

                        let end_offset = if index < length {
                            characters[index].0
                        } else {
                            self.source.len()
                        };
                        let end_location = Location::new(line, column);

                        if terminated {
                            tokens.push(Token::new(
                                TokenKind::FStringLiteral(raw_content),
                                Span::new(offset, end_offset, start_location, end_location),
                            ));
                        } else {
                            errors.push(
                                LexError::new(
                                    "unterminated f-string literal",
                                    Span::new(offset, self.source.len(), start_location, end_location),
                                )
                                .with_hint("Close the f-string with a matching \" character."),
                            );
                        }
                        continue;
                    }

                    // Regular identifier or keyword
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
                    // Scan character by character so we can handle escape sequences
                    // and correctly find the closing quote.
                    let mut string_value = String::new();
                    let mut scan = index + 1; // start after the opening `"`
                    bump_position('"', &mut line, &mut column);
                    let mut terminated = false;

                    while scan < length {
                        let (_, sc) = characters[scan];
                        if sc == '\\' && scan + 1 < length {
                            // Escape sequence
                            let (_, escaped) = characters[scan + 1];
                            bump_position('\\', &mut line, &mut column);
                            bump_position(escaped, &mut line, &mut column);
                            match escaped {
                                '"' => string_value.push('"'),
                                '\\' => string_value.push('\\'),
                                'n' => string_value.push('\n'),
                                't' => string_value.push('\t'),
                                'r' => string_value.push('\r'),
                                '0' => string_value.push('\0'),
                                other => {
                                    // Unknown escape: keep as-is
                                    string_value.push('\\');
                                    string_value.push(other);
                                }
                            }
                            scan += 2;
                        } else if sc == '"' {
                            // Closing quote
                            bump_position('"', &mut line, &mut column);
                            scan += 1;
                            terminated = true;
                            break;
                        } else if sc == '\n' {
                            // Newline inside string — still consume but note it
                            bump_position('\n', &mut line, &mut column);
                            string_value.push('\n');
                            scan += 1;
                        } else {
                            bump_position(sc, &mut line, &mut column);
                            string_value.push(sc);
                            scan += 1;
                        }
                    }

                    let end_offset = if scan < length {
                        characters[scan].0
                    } else {
                        self.source.len()
                    };
                    let end_location = Location::new(line, column);

                    if terminated {
                        tokens.push(Token::new(
                            TokenKind::StringLiteral(string_value),
                            Span::new(offset, end_offset, start_location, end_location),
                        ));
                    } else {
                        errors.push(
                            LexError::new(
                                "unterminated string literal",
                                Span::new(offset, self.source.len(), start_location, end_location),
                            )
                            .with_hint("Close the string with a matching \" character."),
                        );
                    }
                    index = scan;
                }
                '\'' => {
                    // Character literal: 'a', '\n', etc.
                    bump_position('\'', &mut line, &mut column);
                    let mut scan = index + 1;
                    let mut char_value: Option<char> = None;
                    let mut terminated = false;

                    if scan < length {
                        let (_, sc) = characters[scan];
                        if sc == '\\' && scan + 1 < length {
                            // Escape sequence
                            let (_, escaped) = characters[scan + 1];
                            bump_position('\\', &mut line, &mut column);
                            bump_position(escaped, &mut line, &mut column);
                            let ch_val = match escaped {
                                '\'' => '\'',
                                '\\' => '\\',
                                'n'  => '\n',
                                't'  => '\t',
                                'r'  => '\r',
                                '0'  => '\0',
                                other => other,
                            };
                            char_value = Some(ch_val);
                            scan += 2;
                        } else if sc != '\'' {
                            bump_position(sc, &mut line, &mut column);
                            char_value = Some(sc);
                            scan += 1;
                        }
                    }

                    if scan < length && characters[scan].1 == '\'' {
                        bump_position('\'', &mut line, &mut column);
                        scan += 1;
                        terminated = true;
                    }

                    let end_offset = if scan < length {
                        characters[scan].0
                    } else {
                        self.source.len()
                    };
                    let end_location = Location::new(line, column);

                    if terminated {
                        if let Some(c) = char_value {
                            tokens.push(Token::new(
                                TokenKind::CharLiteral(c),
                                Span::new(offset, end_offset, start_location, end_location),
                            ));
                        } else {
                            errors.push(
                                LexError::new(
                                    "empty character literal",
                                    Span::new(offset, end_offset, start_location, end_location),
                                )
                                .with_hint("Character literals must contain exactly one character."),
                            );
                        }
                    } else {
                        errors.push(
                            LexError::new(
                                "unterminated character literal",
                                Span::new(offset, self.source.len(), start_location, end_location),
                            )
                            .with_hint("Close the character literal with a matching ' character."),
                        );
                    }
                    index = scan;
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
                            (TokenKind::Operator(Operator::EqualEqual), 2)
                        }
                        ('=', Some('>')) => {
                            (TokenKind::Operator(Operator::FatArrow), 2)
                        }
                        ('!', Some('=')) => {
                            (TokenKind::Operator(Operator::NotEqual), 2)
                        }
                        ('<', Some('=')) => {
                            (TokenKind::Operator(Operator::LessEqual), 2)
                        }
                        ('>', Some('=')) => {
                            (TokenKind::Operator(Operator::GreaterEqual), 2)
                        }
                        ('&', Some('&')) => (TokenKind::Operator(Operator::And), 2),
                        ('|', Some('|')) => (TokenKind::Operator(Operator::Or), 2),
                        ('-', Some('>')) => (TokenKind::Operator(Operator::Arrow), 2),
                        // Range operators: ..= and ..
                        ('.', Some('.')) => {
                            let third = if index + 2 < length { Some(characters[index + 2].1) } else { None };
                            if third == Some('=') {
                                (TokenKind::Operator(Operator::RangeInclusive), 3)
                            } else {
                                (TokenKind::Operator(Operator::Range), 2)
                            }
                        }
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
                    errors.push(
                        LexError::new(
                            format!("unexpected character `{}`", ch),
                            Span::new(offset, end_offset, start_location, end_location),
                        )
                        .with_hint(
                            "Remove this character or escape it if you intended it to appear literally.",
                        ),
                    );
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
            | '?'
    )
}

/// Advances the line/column cursor by one character.
///
/// **Column convention**: `column` always points to the position *after* the
/// last consumed character — i.e. the column where the *next* character will
/// land.  This makes `end_location` in each `Span` an **exclusive** bound:
/// `end_location.column` is one past the final column of the token.
/// Consumers should display `end_location.column - 1` when they need an
/// inclusive end, or use `end_location` as-is for half-open ranges.
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
