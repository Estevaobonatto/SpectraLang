use crate::{
    ast::{BinaryOperator, Expr, Literal, Module, Stmt, UnaryOperator},
    error::{ParseError, ParseResult},
    span::{Location, Span},
    token::{Keyword, Token, TokenKind},
};

pub struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
        }
    }

    pub fn parse(mut self) -> ParseResult<Module> {
        let mut declarations = Vec::new();
        while !self.is_at_end() {
            match self.declaration() {
                Some(stmt) => declarations.push(stmt),
                None => self.synchronize(),
            }
        }

        if self.errors.is_empty() {
            Ok(Module::new(declarations))
        } else {
            Err(self.errors)
        }
    }

    fn declaration(&mut self) -> Option<Stmt> {
        if self.match_keyword(Keyword::Let) {
            return self.parse_let(false);
        }
        if self.match_keyword(Keyword::Var) {
            return self.parse_let(true);
        }
        self.statement()
    }

    fn parse_let(&mut self, mutable: bool) -> Option<Stmt> {
        let keyword_span = self.previous().span;
        let name_token = self.consume_identifier("expected identifier after let")?;

        self.consume(TokenKind::Equal, "expected '=' after identifier")?;

        let value = self.expression()?;

        let semicolon = self.consume(TokenKind::Semicolon, "expected ';' after declaration")?;
        let stmt_span = span_union(keyword_span, semicolon.span);

        Some(Stmt::Let {
            mutable,
            name: name_token.lexeme.clone(),
            value,
            span: stmt_span,
        })
    }

    fn statement(&mut self) -> Option<Stmt> {
        let expr = self.expression()?;
        self.consume(
            TokenKind::Semicolon,
            "expected ';' after expression statement",
        )?;
        Some(Stmt::Expr(expr))
    }

    fn expression(&mut self) -> Option<Expr> {
        self.parse_equality()
    }

    fn parse_equality(&mut self) -> Option<Expr> {
        let mut expr = self.parse_comparison()?;
        while self.match_any(&[TokenKind::EqualEqual, TokenKind::BangEqual]) {
            let operator = match self.previous().kind {
                TokenKind::EqualEqual => BinaryOperator::Equal,
                TokenKind::BangEqual => BinaryOperator::NotEqual,
                _ => unreachable!(),
            };
            let right = self.parse_comparison()?;
            let span = span_union(expr_span(&expr), expr_span(&right));
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                span,
            };
        }
        Some(expr)
    }

    fn parse_comparison(&mut self) -> Option<Expr> {
        let mut expr = self.parse_term()?;
        while self.match_any(&[
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
        ]) {
            let operator = match self.previous().kind {
                TokenKind::Greater => BinaryOperator::Greater,
                TokenKind::GreaterEqual => BinaryOperator::GreaterEqual,
                TokenKind::Less => BinaryOperator::Less,
                TokenKind::LessEqual => BinaryOperator::LessEqual,
                _ => unreachable!(),
            };
            let right = self.parse_term()?;
            let span = span_union(expr_span(&expr), expr_span(&right));
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                span,
            };
        }
        Some(expr)
    }

    fn parse_term(&mut self) -> Option<Expr> {
        let mut expr = self.parse_factor()?;
        while self.match_any(&[TokenKind::Plus, TokenKind::Minus]) {
            let operator = match self.previous().kind {
                TokenKind::Plus => BinaryOperator::Add,
                TokenKind::Minus => BinaryOperator::Sub,
                _ => unreachable!(),
            };
            let right = self.parse_factor()?;
            let span = span_union(expr_span(&expr), expr_span(&right));
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                span,
            };
        }
        Some(expr)
    }

    fn parse_factor(&mut self) -> Option<Expr> {
        let mut expr = self.parse_unary()?;
        while self.match_any(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let operator = match self.previous().kind {
                TokenKind::Star => BinaryOperator::Mul,
                TokenKind::Slash => BinaryOperator::Div,
                TokenKind::Percent => BinaryOperator::Mod,
                _ => unreachable!(),
            };
            let right = self.parse_unary()?;
            let span = span_union(expr_span(&expr), expr_span(&right));
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
                span,
            };
        }
        Some(expr)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        if self.match_any(&[TokenKind::Bang, TokenKind::Minus]) {
            let operator_token = self.previous().clone();
            let operand = self.parse_unary()?;
            let operator = match operator_token.kind {
                TokenKind::Bang => UnaryOperator::Not,
                TokenKind::Minus => UnaryOperator::Negate,
                _ => unreachable!(),
            };
            let span = span_union(operator_token.span, expr_span(&operand));
            return Some(Expr::Unary {
                operator,
                operand: Box::new(operand),
                span,
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        if self.match_token(TokenKind::Integer) {
            let token = self.previous().clone();
            let value = token
                .lexeme
                .replace('_', "")
                .parse::<i64>()
                .map_err(|_| {
                    self.errors.push(ParseError::new(
                        "invalid integer literal",
                        token.span.start_location,
                    ));
                })
                .ok()?;
            return Some(Expr::Literal {
                value: Literal::Integer(value),
                span: token.span,
            });
        }

        if self.match_token(TokenKind::Float) {
            let token = self.previous().clone();
            let value = token
                .lexeme
                .parse::<f64>()
                .map_err(|_| {
                    self.errors.push(ParseError::new(
                        "invalid float literal",
                        token.span.start_location,
                    ));
                })
                .ok()?;
            return Some(Expr::Literal {
                value: Literal::Float(value),
                span: token.span,
            });
        }

        if self.match_token(TokenKind::String) {
            let token = self.previous().clone();
            return Some(Expr::Literal {
                value: Literal::String(token.lexeme.clone()),
                span: token.span,
            });
        }

        if self.match_keyword(Keyword::True) {
            let token = self.previous().clone();
            return Some(Expr::Literal {
                value: Literal::Bool(true),
                span: token.span,
            });
        }

        if self.match_keyword(Keyword::False) {
            let token = self.previous().clone();
            return Some(Expr::Literal {
                value: Literal::Bool(false),
                span: token.span,
            });
        }

        if self.match_token(TokenKind::Identifier) {
            let token = self.previous().clone();
            return Some(Expr::Identifier {
                name: token.lexeme.clone(),
                span: token.span,
            });
        }

        if self.match_token(TokenKind::LParen) {
            let expr = self.expression()?;
            self.consume(TokenKind::RParen, "expected ')' after expression")?;
            let span = span_union(expr_span(&expr), self.previous().span);
            return Some(Expr::Grouping {
                expression: Box::new(expr),
                span,
            });
        }

        let location = self.peek().span.start_location;
        self.errors
            .push(ParseError::new("expected expression", location));
        None
    }

    fn match_keyword(&mut self, keyword: Keyword) -> bool {
        if let TokenKind::Keyword(current) = self.peek().kind {
            if current == keyword {
                self.advance();
                return true;
            }
        }
        false
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_any(&mut self, kinds: &[TokenKind]) -> bool {
        for &kind in kinds {
            if self.check(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        self.peek().kind == kind
    }

    fn consume(&mut self, kind: TokenKind, message: &str) -> Option<&Token> {
        if self.check(kind) {
            Some(self.advance())
        } else {
            let location = self.peek().span.start_location;
            self.errors.push(ParseError::new(message, location));
            None
        }
    }

    fn consume_identifier(&mut self, message: &str) -> Option<&Token> {
        if self.check(TokenKind::Identifier) {
            Some(self.advance())
        } else {
            let location = self.peek().span.start_location;
            self.errors.push(ParseError::new(message, location));
            None
        }
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn synchronize(&mut self) {
        if self.current == 0 {
            self.current = 1.min(self.tokens.len().saturating_sub(1));
        }

        while !self.is_at_end() {
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }

            match self.peek().kind {
                TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::Var)
                | TokenKind::Keyword(Keyword::Fn)
                | TokenKind::Keyword(Keyword::Class)
                | TokenKind::Keyword(Keyword::Struct) => return,
                _ => {
                    if self.current < self.tokens.len().saturating_sub(1) {
                        self.current += 1;
                    } else {
                        return;
                    }
                }
            }
        }
    }
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Literal { span, .. }
        | Expr::Identifier { span, .. }
        | Expr::Unary { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Grouping { span, .. } => *span,
    }
}

fn span_union(lhs: Span, rhs: Span) -> Span {
    let (start_span, end_span) = if lhs.start <= rhs.start {
        (lhs, rhs)
    } else {
        (rhs, lhs)
    };
    Span::new(
        start_span.start,
        end_span.end,
        start_span.start_location,
        end_span.end_location,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_ok(source: &str) -> Module {
        let tokens = Lexer::new(source).tokenize().expect("lex ok");
        Parser::new(&tokens).parse().expect("parse ok")
    }

    #[test]
    fn parse_let_and_expression() {
        let module = parse_ok("let x = 10; x + 2;");
        assert_eq!(module.declarations.len(), 2);
    }

    #[test]
    fn parse_boolean_and_grouping() {
        let module = parse_ok("let flag = true; (flag);");
        assert_eq!(module.declarations.len(), 2);
    }
}
