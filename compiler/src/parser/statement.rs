use crate::{
    ast::{
        DoWhileLoop, ForLoop, LetStatement, LoopStatement, ReturnStatement, Statement,
        StatementKind, SwitchCase, SwitchStatement, WhileLoop,
    },
    span::span_union,
    token::{Keyword, Operator, TokenKind},
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
            crate::token::TokenKind::Keyword(Keyword::Do) => {
                let feature_span = self.current().span;
                if self
                    .require_feature("do-while", feature_span, "Do-while loops")
                    .is_err()
                {
                    self.synchronize();
                    return Err(());
                }
                self.advance(); // consume 'do'
                self.parse_do_while_statement()?
            }
            crate::token::TokenKind::Keyword(Keyword::For) => {
                self.advance(); // consume 'for'
                self.parse_for_statement()?
            }
            crate::token::TokenKind::Keyword(Keyword::Loop) => {
                let feature_span = self.current().span;
                if self
                    .require_feature("loop", feature_span, "Loop statements")
                    .is_err()
                {
                    self.synchronize();
                    return Err(());
                }
                self.advance(); // consume 'loop'
                self.parse_loop_statement()?
            }
            crate::token::TokenKind::Keyword(Keyword::Switch) => {
                let feature_span = self.current().span;
                if self
                    .require_feature("switch", feature_span, "Switch statements")
                    .is_err()
                {
                    self.synchronize();
                    return Err(());
                }
                self.advance(); // consume 'switch'
                self.parse_switch_statement()?
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
                // Try to parse as assignment or expression statement
                let expr = self.parse_expression()?;

                // Check if followed by '='
                if self.check_symbol('=') {
                    // This is an assignment
                    self.advance(); // consume '='

                    // Convert expression to LValue
                    let (target, target_span) = match expr.kind {
                        crate::ast::ExpressionKind::Identifier(name) => {
                            (crate::ast::LValue::Identifier(name), expr.span)
                        }
                        crate::ast::ExpressionKind::IndexAccess { array, index } => {
                            (crate::ast::LValue::IndexAccess { array, index }, expr.span)
                        }
                        _ => {
                            self.error("Invalid assignment target");
                            return Err(());
                        }
                    };

                    let value = self.parse_expression()?;
                    self.consume_symbol(';', "Expected ';' after assignment")?;

                    StatementKind::Assignment(crate::ast::AssignmentStatement {
                        target,
                        target_span,
                        value,
                    })
                } else {
                    // Expression statement

                    // Only require semicolon if the expression is not a block-ending structure
                    // or if this is not the last expression in a block (next token is not '}')
                    let requires_semicolon = !matches!(
                        expr.kind,
                        crate::ast::ExpressionKind::If { .. }
                            | crate::ast::ExpressionKind::Unless { .. }
                    ) && !self.check_symbol('}');

                    if requires_semicolon {
                        self.consume_symbol(';', "Expected ';' after expression")?;
                    }

                    StatementKind::Expression(expr)
                }
            }
        };

        let end_span = self
            .tokens
            .get(self.position.saturating_sub(1))
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
        let start_span = self
            .tokens
            .get(self.position.saturating_sub(1))
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
        let start_span = self
            .tokens
            .get(self.position.saturating_sub(1))
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
        let start_span = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.current().span);

        let (iterator, _) = self.consume_identifier("Expected iterator variable name")?;

        // Accept 'in' or 'of' — both have identical semantics
        if self.check_keyword(Keyword::In) || self.check_keyword(Keyword::Of) {
            self.advance();
        } else {
            self.error("Expected 'in' or 'of' after iterator variable");
            return Err(());
        }

        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(StatementKind::For(ForLoop {
            iterator,
            iterable,
            body,
            span: span_union(start_span, end_span),
        }))
    }

    fn parse_loop_statement(&mut self) -> Result<StatementKind, ()> {
        // loop { ... }
        let start_span = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.current().span);
        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(StatementKind::Loop(LoopStatement {
            body,
            span: span_union(start_span, end_span),
        }))
    }

    fn parse_do_while_statement(&mut self) -> Result<StatementKind, ()> {
        // do { ... } while condition;
        let start_span = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.current().span);
        let body = self.parse_block()?;

        if !self.check_keyword(Keyword::While) {
            self.error("Expected 'while' after do-block");
            return Err(());
        }
        self.advance(); // consume 'while'

        let condition = self.parse_expression()?;
        let end_span = condition.span;
        self.consume_symbol(';', "Expected ';' after do-while condition")?;

        Ok(StatementKind::DoWhile(DoWhileLoop {
            body,
            condition,
            span: span_union(start_span, end_span),
        }))
    }

    fn parse_switch_statement(&mut self) -> Result<StatementKind, ()> {
        // switch value { case pattern => body, ... default => body }
        let start_span = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map(|t| t.span)
            .unwrap_or(self.current().span);
        let value = self.parse_expression()?;

        self.consume_symbol('{', "Expected '{' after switch value")?;

        let mut cases = Vec::new();
        let mut default = None;

        while !self.check_symbol('}') && self.position < self.tokens.len() {
            if self.check_keyword(Keyword::Case) {
                self.advance(); // consume 'case'
                let pattern = self.parse_expression()?;

                // Aceita ':' ou '=>' como separador
                if self.check_symbol(':') {
                    self.advance();
                } else if matches!(self.current().kind, TokenKind::Operator(Operator::FatArrow)) {
                    self.advance(); // consume '=>'
                } else {
                    self.error("Expected ':' or '=>' after case pattern");
                    return Err(());
                }

                let case_body = self.parse_block()?;
                let case_span = span_union(pattern.span, case_body.span);

                cases.push(SwitchCase {
                    pattern,
                    body: case_body,
                    span: case_span,
                });
            } else if self.check_keyword(Keyword::Else) {
                self.advance(); // consume 'else'

                // Aceita ':' ou '=>'
                if self.check_symbol(':') {
                    self.advance();
                } else if matches!(self.current().kind, TokenKind::Operator(Operator::FatArrow)) {
                    self.advance(); // consume '=>'
                }

                default = Some(self.parse_block()?);
                break; // default deve ser o último
            } else {
                self.error("Expected 'case' or 'default' in switch body");
                return Err(());
            }
        }

        let end_span = self.current().span;
        self.consume_symbol('}', "Expected '}' to close switch statement")?;

        Ok(StatementKind::Switch(SwitchStatement {
            value,
            cases,
            default,
            span: span_union(start_span, end_span),
        }))
    }
}
