use crate::{
    ast::{ForLoop, LetStatement, ReturnStatement, Statement, StatementKind, WhileLoop},
    span::span_union,
    token::Keyword,
};

use super::Parser;

impl Parser {
    pub(super) fn parse_statement(&mut self) -> Result<Statement, ()> {
        let start_span = self.current().span;

        let kind = match &self.current().kind {
            crate::token::TokenKind::Keyword(Keyword::Let) => {
                self.advance(); // consume 'let'
                self.parse_let_statement()?
            }
            crate::token::TokenKind::Keyword(Keyword::Return) => {
                self.advance(); // consume 'return'
                self.parse_return_statement()?
            }
            crate::token::TokenKind::Keyword(Keyword::While) => {
                self.advance(); // consume 'while'
                self.parse_while_statement()?
            }
            crate::token::TokenKind::Keyword(Keyword::For) => {
                self.advance(); // consume 'for'
                self.parse_for_statement()?
            }
            crate::token::TokenKind::Keyword(Keyword::Break) => {
                self.advance(); // consume 'break'
                self.consume_symbol(';', "Expected ';' after 'break'")?;
                StatementKind::Break
            }
            crate::token::TokenKind::Keyword(Keyword::Continue) => {
                self.advance(); // consume 'continue'
                self.consume_symbol(';', "Expected ';' after 'continue'")?;
                StatementKind::Continue
            }
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                self.consume_symbol(';', "Expected ';' after expression")?;
                StatementKind::Expression(expr)
            }
        };

        let end_span = self.tokens.get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(start_span);

        Ok(Statement {
            span: span_union(start_span, end_span),
            kind,
        })
    }

    fn parse_let_statement(&mut self) -> Result<StatementKind, ()> {
        // Expect: let <name> [: type] [= expr];
        let (name, name_span) = self.consume_identifier("Expected variable name after 'let'")?;

        // Optional type annotation
        let ty = if self.check_symbol(':') {
            self.advance(); // consume ':'
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        // Optional initializer
        let value = if self.check_symbol('=') {
            self.advance(); // consume '='
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.consume_symbol(';', "Expected ';' after let statement")?;

        Ok(StatementKind::Let(LetStatement {
            name,
            span: name_span,
            ty,
            value,
        }))
    }

    fn parse_return_statement(&mut self) -> Result<StatementKind, ()> {
        // Expect: return [expr];
        let start_span = self.tokens.get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.current().span);

        let value = if !self.check_symbol(';') {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.consume_symbol(';', "Expected ';' after return statement")?;

        Ok(StatementKind::Return(ReturnStatement {
            span: start_span,
            value,
        }))
    }

    fn parse_while_statement(&mut self) -> Result<StatementKind, ()> {
        // Expect: while <condition> { <body> }
        let start_span = self.tokens.get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.current().span);

        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(StatementKind::While(WhileLoop {
            condition,
            body,
            span: span_union(start_span, end_span),
        }))
    }

    fn parse_for_statement(&mut self) -> Result<StatementKind, ()> {
        // Expect: for <iterator> in/of <iterable> { <body> }
        let start_span = self.tokens.get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.current().span);

        let (iterator, _) = self.consume_identifier("Expected iterator variable name")?;

        // Check for 'in' or 'of'
        let is_in = if self.check_keyword(Keyword::In) {
            self.advance();
            true
        } else if self.check_keyword(Keyword::Of) {
            self.advance();
            false
        } else {
            self.error("Expected 'in' or 'of' after iterator variable");
            return Err(());
        };

        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(StatementKind::For(ForLoop {
            iterator,
            iterable,
            body,
            span: span_union(start_span, end_span),
            is_in,
        }))
    }
}
