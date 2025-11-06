use crate::{
    ast::{BinaryOperator, Expression, ExpressionKind, UnaryOperator},
    token::{Keyword, Operator, TokenKind},
};

use super::Parser;

impl Parser {
    pub(super) fn parse_expression(&mut self) -> Result<Expression, ()> {
        self.parse_logical_or()
    }

    // Logical OR (lowest precedence)
    fn parse_logical_or(&mut self) -> Result<Expression, ()> {
        let mut left = self.parse_logical_and()?;

        while matches!(&self.current().kind, TokenKind::Operator(Operator::Or)) {
            self.advance();
            let right = self.parse_logical_and()?;
            let span = crate::span::span_union(left.span, right.span);
            left = Expression {
                span,
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator: BinaryOperator::Or,
                    right: Box::new(right),
                },
            };
        }

        Ok(left)
    }

    // Logical AND
    fn parse_logical_and(&mut self) -> Result<Expression, ()> {
        let mut left = self.parse_equality()?;

        while matches!(&self.current().kind, TokenKind::Operator(Operator::And)) {
            self.advance();
            let right = self.parse_equality()?;
            let span = crate::span::span_union(left.span, right.span);
            left = Expression {
                span,
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator: BinaryOperator::And,
                    right: Box::new(right),
                },
            };
        }

        Ok(left)
    }

    // Equality (==, !=)
    fn parse_equality(&mut self) -> Result<Expression, ()> {
        let mut left = self.parse_comparison()?;

        loop {
            let operator = match &self.current().kind {
                TokenKind::Operator(Operator::EqualEqual) => BinaryOperator::Equal,
                TokenKind::Operator(Operator::NotEqual) => BinaryOperator::NotEqual,
                _ => break,
            };

            self.advance();
            let right = self.parse_comparison()?;
            let span = crate::span::span_union(left.span, right.span);
            left = Expression {
                span,
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
            };
        }

        Ok(left)
    }

    // Comparison (<, >, <=, >=)
    fn parse_comparison(&mut self) -> Result<Expression, ()> {
        let mut left = self.parse_addition()?;

        loop {
            let operator = match &self.current().kind {
                TokenKind::Symbol('<') => BinaryOperator::Less,
                TokenKind::Symbol('>') => BinaryOperator::Greater,
                TokenKind::Operator(Operator::LessEqual) => BinaryOperator::LessEqual,
                TokenKind::Operator(Operator::GreaterEqual) => BinaryOperator::GreaterEqual,
                _ => break,
            };

            self.advance();
            let right = self.parse_addition()?;
            let span = crate::span::span_union(left.span, right.span);
            left = Expression {
                span,
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
            };
        }

        Ok(left)
    }

    // Addition and Subtraction
    fn parse_addition(&mut self) -> Result<Expression, ()> {
        let mut left = self.parse_multiplication()?;

        loop {
            let operator = match &self.current().kind {
                TokenKind::Symbol('+') => BinaryOperator::Add,
                TokenKind::Symbol('-') => BinaryOperator::Subtract,
                _ => break,
            };

            self.advance();
            let right = self.parse_multiplication()?;
            let span = crate::span::span_union(left.span, right.span);
            left = Expression {
                span,
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
            };
        }

        Ok(left)
    }

    // Multiplication, Division, Modulo
    fn parse_multiplication(&mut self) -> Result<Expression, ()> {
        let mut left = self.parse_unary()?;

        loop {
            let operator = match &self.current().kind {
                TokenKind::Symbol('*') => BinaryOperator::Multiply,
                TokenKind::Symbol('/') => BinaryOperator::Divide,
                TokenKind::Symbol('%') => BinaryOperator::Modulo,
                _ => break,
            };

            self.advance();
            let right = self.parse_unary()?;
            let span = crate::span::span_union(left.span, right.span);
            left = Expression {
                span,
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
            };
        }

        Ok(left)
    }

    // Unary expressions (-, !)
    fn parse_unary(&mut self) -> Result<Expression, ()> {
        let operator = match &self.current().kind {
            TokenKind::Symbol('-') => Some(UnaryOperator::Negate),
            TokenKind::Symbol('!') => Some(UnaryOperator::Not),
            _ => None,
        };

        if let Some(op) = operator {
            let start_span = self.current().span;
            self.advance();
            let operand = self.parse_unary()?;
            let span = crate::span::span_union(start_span, operand.span);
            Ok(Expression {
                span,
                kind: ExpressionKind::Unary {
                    operator: op,
                    operand: Box::new(operand),
                },
            })
        } else {
            self.parse_call_expression()
        }
    }

    fn parse_call_expression(&mut self) -> Result<Expression, ()> {
        let mut expr = self.parse_primary_expression()?;

        // Handle function calls and array indexing
        loop {
            if self.check_symbol('(') {
                self.advance(); // consume '('

                let mut arguments = Vec::new();

                if !self.check_symbol(')') {
                    loop {
                        arguments.push(self.parse_expression()?);
                        if !self.check_symbol(',') {
                            break;
                        }
                        self.advance(); // consume ','
                    }
                }

                let end_span = self.consume_symbol(')', "Expected ')' after arguments")?;

                let span = crate::span::span_union(expr.span, end_span);
                expr = Expression {
                    span,
                    kind: ExpressionKind::Call {
                        callee: Box::new(expr),
                        arguments,
                    },
                };
            } else if self.check_symbol('[') {
                self.advance(); // consume '['
                let index = self.parse_expression()?;
                let end_span = self.consume_symbol(']', "Expected ']' after index")?;

                let span = crate::span::span_union(expr.span, end_span);
                expr = Expression {
                    span,
                    kind: ExpressionKind::IndexAccess {
                        array: Box::new(expr),
                        index: Box::new(index),
                    },
                };
            } else if self.check_symbol('.') {
                self.advance(); // consume '.'

                // Check if it's a number (tuple access) or identifier (field access)
                if let TokenKind::Number(num_str) = &self.current().kind {
                    // Tuple access: .0, .1, .2, etc.
                    if let Ok(index) = num_str.parse::<usize>() {
                        let end_span = self.current().span;
                        self.advance();

                        let span = crate::span::span_union(expr.span, end_span);
                        expr = Expression {
                            span,
                            kind: ExpressionKind::TupleAccess {
                                tuple: Box::new(expr),
                                index,
                            },
                        };
                    } else {
                        self.error("Invalid tuple index");
                        return Err(());
                    }
                } else if let TokenKind::Identifier(_) = &self.current().kind {
                    // Field access ou method call: .field_name ou .method_name(args)
                    let (name, name_span) =
                        self.consume_identifier("Expected field/method name after '.'")?;

                    // Verificar se é method call (seguido de '(')
                    if self.check_symbol('(') {
                        self.advance(); // consume '('

                        let mut arguments = Vec::new();
                        if !self.check_symbol(')') {
                            loop {
                                arguments.push(self.parse_expression()?);
                                if !self.check_symbol(',') {
                                    break;
                                }
                                self.advance(); // consume ','
                            }
                        }

                        let end_span =
                            self.consume_symbol(')', "Expected ')' after method arguments")?;

                        let span = crate::span::span_union(expr.span, end_span);
                        expr = Expression {
                            span,
                            kind: ExpressionKind::MethodCall {
                                object: Box::new(expr),
                                method_name: name,
                                arguments,
                                type_name: None, // Será preenchido pelo semantic analyzer
                            },
                        };
                    } else {
                        // É field access
                        let span = crate::span::span_union(expr.span, name_span);
                        expr = Expression {
                            span,
                            kind: ExpressionKind::FieldAccess {
                                object: Box::new(expr),
                                field: name,
                            },
                        };
                    }
                } else {
                    self.error("Expected number or field name after '.'");
                    return Err(());
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary_expression(&mut self) -> Result<Expression, ()> {
        let token = self.current();
        let span = token.span;

        match &token.kind {
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expression {
                    span,
                    kind: ExpressionKind::BoolLiteral(true),
                })
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expression {
                    span,
                    kind: ExpressionKind::BoolLiteral(false),
                })
            }
            TokenKind::Keyword(Keyword::If) => self.parse_if_expression(),
            TokenKind::Keyword(Keyword::Unless) => self.parse_unless_expression(),
            TokenKind::Keyword(Keyword::Match) => self.parse_match_expression(),
            TokenKind::Identifier(name) => {
                let name = name.clone();
                let start_span = span;
                self.advance();

                // Parse optional type arguments: Name<Type1, Type2>
                let type_args = if self.check_symbol('<') {
                    self.parse_type_arguments()?
                } else {
                    Vec::new()
                };

                // Check if it's an enum variant: Name::Variant or Name::Variant(data)
                if self.check_symbol(':')
                    && self.position + 1 < self.tokens.len()
                    && matches!(self.tokens[self.position + 1].kind, TokenKind::Symbol(':'))
                {
                    self.advance(); // consume first ':'
                    self.advance(); // consume second ':'

                    let (variant_name, _) =
                        self.consume_identifier("Expected variant name after '::")?;

                    // Check for tuple variant data
                    let data = if self.check_symbol('(') {
                        self.advance(); // consume '('

                        let mut args = Vec::new();
                        if !self.check_symbol(')') {
                            loop {
                                args.push(self.parse_expression()?);
                                if !self.check_symbol(',') {
                                    break;
                                }
                                self.advance(); // consume ','
                            }
                        }

                        let _end_span =
                            self.consume_symbol(')', "Expected ')' after variant data")?;
                        Some(args)
                    } else {
                        None
                    };

                    return Ok(Expression {
                        span: crate::span::span_union(start_span, self.current().span),
                        kind: ExpressionKind::EnumVariant {
                            enum_name: name,
                            type_args,
                            variant_name,
                            data,
                        },
                    });
                }

                // Check if it's a struct literal: Name { fields }
                // Only if followed by { and then identifier:value pattern
                if self.check_symbol('{') {
                    // Lookahead: after '{', should see identifier followed by SINGLE ':'
                    // (not '::' which would be enum variant)
                    let is_struct_literal = if self.position + 1 < self.tokens.len() {
                        let has_identifier = matches!(
                            self.tokens[self.position + 1].kind,
                            TokenKind::Identifier(_)
                        );
                        let has_colon = self.position + 2 < self.tokens.len()
                            && matches!(
                                self.tokens[self.position + 2].kind,
                                TokenKind::Symbol(':')
                            );
                        // Make sure it's NOT followed by another colon (::)
                        let not_double_colon = self.position + 3 >= self.tokens.len()
                            || !matches!(
                                self.tokens[self.position + 3].kind,
                                TokenKind::Symbol(':')
                            );
                        has_identifier && has_colon && not_double_colon
                    } else {
                        false
                    };

                    if is_struct_literal {
                        self.advance(); // consume '{'

                        let mut fields = Vec::new();

                        // Parse fields
                        while !self.check_symbol('}') && !self.is_at_end() {
                            // Parse: field_name: value
                            let (field_name, _) = self.consume_identifier("Expected field name")?;
                            self.consume_symbol(':', "Expected ':' after field name")?;
                            let field_value = self.parse_expression()?;

                            fields.push((field_name, field_value));

                            // Optional comma
                            if self.check_symbol(',') {
                                self.advance();
                            }
                        }

                        let end_span = self.current().span;
                        self.consume_symbol('}', "Expected '}' to end struct literal")?;

                        Ok(Expression {
                            span: crate::span::span_union(start_span, end_span),
                            kind: ExpressionKind::StructLiteral {
                                name,
                                type_args,
                                fields,
                            },
                        })
                    } else {
                        // Just an identifier, '{' belongs to surrounding context
                        Ok(Expression {
                            span,
                            kind: ExpressionKind::Identifier(name),
                        })
                    }
                } else {
                    // Just an identifier
                    Ok(Expression {
                        span,
                        kind: ExpressionKind::Identifier(name),
                    })
                }
            }
            TokenKind::Number(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression {
                    span,
                    kind: ExpressionKind::NumberLiteral(value),
                })
            }
            TokenKind::StringLiteral(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression {
                    span,
                    kind: ExpressionKind::StringLiteral(value),
                })
            }
            TokenKind::Symbol('(') => {
                self.advance(); // consume '('

                // Check for empty tuple ()
                if self.check_symbol(')') {
                    let end_span = self.current().span;
                    self.advance();
                    let span = crate::span::span_union(span, end_span);
                    return Ok(Expression {
                        span,
                        kind: ExpressionKind::TupleLiteral { elements: vec![] },
                    });
                }

                let first_expr = self.parse_expression()?;

                // If followed by comma, it's a tuple
                if self.check_symbol(',') {
                    let mut elements = vec![first_expr];

                    while self.check_symbol(',') {
                        self.advance(); // consume ','

                        // Allow trailing comma before ')'
                        if self.check_symbol(')') {
                            break;
                        }

                        elements.push(self.parse_expression()?);
                    }

                    let end_span = self.consume_symbol(')', "Expected ')' after tuple elements")?;
                    let span = crate::span::span_union(span, end_span);

                    Ok(Expression {
                        span,
                        kind: ExpressionKind::TupleLiteral { elements },
                    })
                } else {
                    // Just grouping
                    self.consume_symbol(')', "Expected ')' after expression")?;
                    Ok(Expression {
                        span,
                        kind: ExpressionKind::Grouping(Box::new(first_expr)),
                    })
                }
            }
            TokenKind::Symbol('[') => {
                self.advance(); // consume '['
                let mut elements = Vec::new();

                if !self.check_symbol(']') {
                    loop {
                        elements.push(self.parse_expression()?);
                        if !self.check_symbol(',') {
                            break;
                        }
                        self.advance(); // consume ','
                    }
                }

                let end_span = self.consume_symbol(']', "Expected ']' after array elements")?;
                let span = crate::span::span_union(span, end_span);

                Ok(Expression {
                    span,
                    kind: ExpressionKind::ArrayLiteral { elements },
                })
            }
            _ => {
                self.error("Expected expression");
                Err(())
            }
        }
    }

    fn parse_if_expression(&mut self) -> Result<Expression, ()> {
        let start_span = self.consume_keyword(Keyword::If, "Expected 'if'")?;

        let condition = Box::new(self.parse_expression()?);
        let then_block = self.parse_block()?;

        let mut elif_blocks = Vec::new();

        // Parse elif/elseif blocks
        while self.check_keyword(Keyword::Elif) || self.check_keyword(Keyword::ElseIf) {
            self.advance(); // consume 'elif' or 'elseif'
            let elif_condition = self.parse_expression()?;
            let elif_body = self.parse_block()?;
            elif_blocks.push((elif_condition, elif_body));
        }

        // Parse optional else block
        let else_block = if self.check_keyword(Keyword::Else) {
            self.advance(); // consume 'else'
            Some(self.parse_block()?)
        } else {
            None
        };

        let end_span = else_block
            .as_ref()
            .map(|b| b.span)
            .or_else(|| elif_blocks.last().map(|(_, b)| b.span))
            .unwrap_or(then_block.span);

        Ok(Expression {
            span: crate::span::span_union(start_span, end_span),
            kind: ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            },
        })
    }

    fn parse_unless_expression(&mut self) -> Result<Expression, ()> {
        // unless condition { body } [else { else_body }]
        // É equivalente a: if !(condition) { body } [else { else_body }]
        let start_span = self.current().span;
        self.advance(); // consume 'unless'

        let condition = Box::new(self.parse_expression()?);
        let then_block = self.parse_block()?;

        // Optional else block
        let else_block = if self.check_keyword(Keyword::Else) {
            self.advance(); // consume 'else'
            Some(self.parse_block()?)
        } else {
            None
        };

        let end_span = else_block
            .as_ref()
            .map(|b| b.span)
            .unwrap_or(then_block.span);

        Ok(Expression {
            span: crate::span::span_union(start_span, end_span),
            kind: ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            },
        })
    }

    fn parse_match_expression(&mut self) -> Result<Expression, ()> {
        use crate::ast::MatchArm;

        let start_span = self.consume_keyword(Keyword::Match, "Expected 'match'")?;
        let scrutinee = Box::new(self.parse_expression()?);

        // Expect '{'
        self.consume_symbol('{', "Expected '{' after match scrutinee")?;

        let mut arms = Vec::new();

        // Parse match arms
        while !self.check_symbol('}') && !self.is_at_end() {
            // Parse pattern
            let pattern = self.parse_pattern()?;

            // Expect '=>' operator
            if !matches!(
                self.current().kind,
                TokenKind::Operator(crate::token::Operator::FatArrow)
            ) {
                self.error("Expected '=>' after pattern in match arm");
                return Err(());
            }
            self.advance(); // consume '=>'

            // Parse body expression
            let body = self.parse_expression()?;

            arms.push(MatchArm { pattern, body });

            // Optional comma, or break if we see '}'
            if self.check_symbol(',') {
                self.advance();
            } else if self.check_symbol('}') {
                break; // End of match arms
            }
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end match")?;

        Ok(Expression {
            span: crate::span::span_union(start_span, end_span),
            kind: ExpressionKind::Match { scrutinee, arms },
        })
    }

    fn parse_pattern(&mut self) -> Result<crate::ast::Pattern, ()> {
        use crate::ast::Pattern;

        // Wildcard pattern: _
        if self.check_symbol('_') {
            self.advance();
            return Ok(Pattern::Wildcard);
        }

        // Check for enum variant pattern: EnumName::VariantName or EnumName<Type>::VariantName
        if let TokenKind::Identifier(name) = &self.current().kind {
            let first_name = name.clone();
            self.advance();

            // Parse optional type arguments: EnumName<Type>
            let type_args = if self.check_symbol('<') {
                self.parse_type_arguments()?
            } else {
                Vec::new()
            };

            // Check for ::
            if self.check_symbol(':')
                && self.position + 1 < self.tokens.len()
                && matches!(self.tokens[self.position + 1].kind, TokenKind::Symbol(':'))
            {
                self.advance(); // consume first ':'
                self.advance(); // consume second ':'

                let (variant_name, _) = self.consume_identifier("Expected variant name")?;

                // Check for data patterns: (pattern, pattern, ...)
                let data = if self.check_symbol('(') {
                    self.advance(); // consume '('

                    let mut patterns = Vec::new();
                    if !self.check_symbol(')') {
                        loop {
                            patterns.push(self.parse_pattern()?);
                            if !self.check_symbol(',') {
                                break;
                            }
                            self.advance(); // consume ','
                        }
                    }

                    self.consume_symbol(')', "Expected ')' after variant data patterns")?;
                    Some(patterns)
                } else {
                    None
                };

                return Ok(Pattern::EnumVariant {
                    enum_name: first_name,
                    type_args,
                    variant_name,
                    data,
                });
            }

            // Just an identifier pattern (binding)
            return Ok(Pattern::Identifier(first_name));
        }

        // Literal patterns (números, booleanos, etc.)
        let expr = self.parse_primary_expression()?;
        Ok(Pattern::Literal(expr))
    }

    /// Parse type arguments: <Type1, Type2>
    /// Used in generic type instantiation: Point<int>, Option<string>
    fn parse_type_arguments(&mut self) -> Result<Vec<crate::ast::TypeAnnotation>, ()> {
        self.consume_symbol('<', "Expected '<' for type arguments")?;

        let mut type_args = Vec::new();

        // Parse first type argument
        if !self.check_symbol('>') {
            loop {
                let type_arg = self.parse_type_annotation()?;
                type_args.push(type_arg);

                if !self.check_symbol(',') {
                    break;
                }
                self.advance(); // consume ','
            }
        }

        self.consume_symbol('>', "Expected '>' after type arguments")?;

        Ok(type_args)
    }
}
