use crate::{
    ast::{
        BinaryOperator, Block, Constant, EnumDecl, EnumVariant, EnumVariantData, Export, Expr,
        Function, Import, Item, Literal, MatchArm, MatchPattern, Module, ModulePath, Parameter,
        Stmt, StructDecl, StructField, StructFieldInit, TypeName, UnaryOperator, Visibility,
    },
    error::{ParseError, ParseResult},
    span::Span,
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
        let mut module_name = None;

        if self.check_keyword(Keyword::Module) {
            let module_token = self.advance().clone();
            module_name = match self.parse_module_declaration(module_token) {
                Some(path) => Some(path),
                None => {
                    self.synchronize();
                    None
                }
            };
        }

        let mut items = Vec::new();
        while !self.is_at_end() {
            if self.match_token(TokenKind::Semicolon) {
                continue;
            }

            if self.check_keyword(Keyword::Export) {
                let export_token = self.advance().clone();
                match self.parse_export(export_token) {
                    Some(export) => items.push(Item::Export(export)),
                    None => self.synchronize(),
                }
                continue;
            }

            if let Some((visibility, span)) = self.try_parse_visibility() {
                if self.check_keyword(Keyword::Fn) {
                    let fn_token = self.advance().clone();
                    match self.parse_function(fn_token, visibility, Some(span)) {
                        Some(function) => items.push(Item::Function(function)),
                        None => self.synchronize(),
                    }
                } else if self.check_keyword(Keyword::Struct) {
                    let struct_token = self.advance().clone();
                    match self.parse_struct(struct_token, visibility, Some(span)) {
                        Some(struct_decl) => items.push(Item::Struct(struct_decl)),
                        None => self.synchronize(),
                    }
                } else if self.check_keyword(Keyword::Enum) {
                    let enum_token = self.advance().clone();
                    match self.parse_enum(enum_token, visibility, Some(span)) {
                        Some(enum_decl) => items.push(Item::Enum(enum_decl)),
                        None => self.synchronize(),
                    }
                } else if self.check_keyword(Keyword::Let) || self.check_keyword(Keyword::Var) {
                    let keyword = self.advance().clone();
                    match self.parse_constant(keyword, visibility, span) {
                        Some(constant) => items.push(Item::Constant(constant)),
                        None => self.synchronize(),
                    }
                } else {
                    let location = self.peek().span.start_location;
                    self.errors.push(ParseError::new(
                        "expected 'fn', 'struct' or binding after visibility modifier",
                        location,
                    ));
                    self.synchronize();
                }
                continue;
            }

            if self.check_keyword(Keyword::Import) {
                let import_token = self.advance().clone();
                match self.parse_import(import_token) {
                    Some(import) => items.push(Item::Import(import)),
                    None => self.synchronize(),
                }
                continue;
            }

            if self.check_keyword(Keyword::Fn) {
                let fn_token = self.advance().clone();
                match self.parse_function(fn_token, Visibility::Private, None) {
                    Some(function) => items.push(Item::Function(function)),
                    None => self.synchronize(),
                }
                continue;
            }

            if self.check_keyword(Keyword::Struct) {
                let struct_token = self.advance().clone();
                match self.parse_struct(struct_token, Visibility::Private, None) {
                    Some(struct_decl) => items.push(Item::Struct(struct_decl)),
                    None => self.synchronize(),
                }
                continue;
            }

            if self.check_keyword(Keyword::Enum) {
                let enum_token = self.advance().clone();
                match self.parse_enum(enum_token, Visibility::Private, None) {
                    Some(enum_decl) => items.push(Item::Enum(enum_decl)),
                    None => self.synchronize(),
                }
                continue;
            }

            if self.check_keyword(Keyword::Module) {
                let token = self.advance().clone();
                self.errors.push(ParseError::new(
                    "module declaration must appear before other items",
                    token.span.start_location,
                ));
                if self.parse_module_declaration(token).is_none() {
                    self.synchronize();
                }
                continue;
            }

            match self.parse_statement() {
                Some(stmt) => items.push(Item::Stmt(stmt)),
                None => self.synchronize(),
            }
        }

        if self.errors.is_empty() {
            Ok(Module::new(module_name, items))
        } else {
            Err(self.errors)
        }
    }

    fn expression(&mut self) -> Option<Expr> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Option<Expr> {
        let mut expr = self.parse_logical_and()?;
        while self.match_token(TokenKind::PipePipe) {
            let right = self.parse_logical_and()?;
            let span = span_union(expr_span(&expr), expr_span(&right));
            expr = Expr::Binary {
                left: Box::new(expr),
                operator: BinaryOperator::Or,
                right: Box::new(right),
                span,
            };
        }
        Some(expr)
    }

    fn parse_logical_and(&mut self) -> Option<Expr> {
        let mut expr = self.parse_equality()?;
        while self.match_token(TokenKind::AmpersandAmpersand) {
            let right = self.parse_equality()?;
            let span = span_union(expr_span(&expr), expr_span(&right));
            expr = Expr::Binary {
                left: Box::new(expr),
                operator: BinaryOperator::And,
                right: Box::new(right),
                span,
            };
        }
        Some(expr)
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        if self.check_keyword(Keyword::For) {
            let keyword = self.advance().clone();
            return self.parse_for_statement(keyword.span);
        }

        if self.check_keyword(Keyword::Match) {
            let keyword = self.advance().clone();
            return self.parse_match_statement(keyword.span);
        }

        if self.check_keyword(Keyword::If) {
            let keyword = self.advance().clone();
            return self.parse_if_statement(keyword.span);
        }

        if self.check_keyword(Keyword::While) {
            let keyword = self.advance().clone();
            return self.parse_while_statement(keyword.span);
        }

        if self.check_keyword(Keyword::Break) {
            let keyword = self.advance().clone();
            return self.parse_break_statement(keyword.span);
        }

        if self.check_keyword(Keyword::Continue) {
            let keyword = self.advance().clone();
            return self.parse_continue_statement(keyword.span);
        }

        if self.check_keyword(Keyword::Return) {
            let keyword = self.advance().clone();
            return self.parse_return(keyword.span);
        }

        if self.check_keyword(Keyword::Let) {
            let keyword = self.advance().clone();
            return self.parse_binding(false, keyword.span);
        }

        if self.check_keyword(Keyword::Var) {
            let keyword = self.advance().clone();
            return self.parse_binding(true, keyword.span);
        }

        if self.match_token(TokenKind::LBrace) {
            let open = self.previous().clone();
            let block = self.parse_block_from_open(open)?;
            return Some(Stmt::Block(block));
        }

        if self.check(TokenKind::Identifier) && matches!(self.peek_next_kind(), TokenKind::Equal) {
            return self.parse_assignment_statement();
        }

        self.parse_expression_statement()
    }

    fn parse_if_statement(&mut self, keyword_span: Span) -> Option<Stmt> {
        self.consume(TokenKind::LParen, "expected '(' after 'if'")?;
        let condition = self.expression()?;
        let close_paren = self
            .consume(TokenKind::RParen, "expected ')' after if condition")?
            .clone();
        let then_branch = self.parse_statement()?;

        let else_branch = if self.check_keyword(Keyword::Else) {
            self.advance();
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };

        let mut span = span_union(keyword_span, expr_span(&condition));
        span = span_union(span, statement_span(&then_branch));
        if let Some(branch) = else_branch.as_deref() {
            span = span_union(span, statement_span(branch));
        }
        span = span_union(span, close_paren.span);

        Some(Stmt::If {
            condition,
            then_branch: Box::new(then_branch),
            else_branch,
            span,
        })
    }

    fn parse_while_statement(&mut self, keyword_span: Span) -> Option<Stmt> {
        self.consume(TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.expression()?;
        let close_paren = self
            .consume(TokenKind::RParen, "expected ')' after while condition")?
            .clone();
        let body = self.parse_statement()?;
        let mut span = span_union(keyword_span, expr_span(&condition));
        span = span_union(span, statement_span(&body));
        span = span_union(span, close_paren.span);

        Some(Stmt::While {
            condition,
            body: Box::new(body),
            span,
        })
    }

    fn parse_break_statement(&mut self, keyword_span: Span) -> Option<Stmt> {
        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after 'break'")?
            .clone();
        let span = span_union(keyword_span, semicolon.span);
        Some(Stmt::Break { span })
    }

    fn parse_continue_statement(&mut self, keyword_span: Span) -> Option<Stmt> {
        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after 'continue'")?
            .clone();
        let span = span_union(keyword_span, semicolon.span);
        Some(Stmt::Continue { span })
    }

    fn parse_for_statement(&mut self, keyword_span: Span) -> Option<Stmt> {
        self.consume(TokenKind::LParen, "expected '(' after 'for'")?;

        let initializer = if self.check(TokenKind::Semicolon) {
            self.advance();
            None
        } else if self.check_keyword(Keyword::Let) {
            let keyword = self.advance().clone();
            Some(Box::new(self.parse_binding(false, keyword.span)?))
        } else if self.check_keyword(Keyword::Var) {
            let keyword = self.advance().clone();
            Some(Box::new(self.parse_binding(true, keyword.span)?))
        } else {
            let expr = self.expression()?;
            self.consume(
                TokenKind::Semicolon,
                "expected ';' after for-loop initializer",
            )?;
            Some(Box::new(Stmt::Expr(expr)))
        };

        let condition = if self.check(TokenKind::Semicolon) {
            self.advance();
            None
        } else {
            let expr = self.expression()?;
            self.consume(
                TokenKind::Semicolon,
                "expected ';' after for-loop condition",
            )?;
            Some(expr)
        };

        let increment = if self.check(TokenKind::RParen) {
            None
        } else {
            Some(self.expression()?)
        };

        let close_paren = self
            .consume(TokenKind::RParen, "expected ')' after for-loop clauses")?
            .clone();
        let body = self.parse_statement()?;

        let mut span = keyword_span;
        if let Some(init) = initializer.as_deref() {
            span = span_union(span, statement_span(init));
        }
        if let Some(cond) = condition.as_ref() {
            span = span_union(span, expr_span(cond));
        }
        if let Some(inc) = increment.as_ref() {
            span = span_union(span, expr_span(inc));
        }
        span = span_union(span, close_paren.span);
        span = span_union(span, statement_span(&body));

        Some(Stmt::For {
            initializer,
            condition,
            increment,
            body: Box::new(body),
            span,
        })
    }

    fn parse_match_statement(&mut self, keyword_span: Span) -> Option<Stmt> {
        let expression = self.expression()?;
        let open_brace = self
            .consume(TokenKind::LBrace, "expected '{' after match expression")?
            .clone();
        let mut arms = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let pattern = self.parse_match_pattern()?;
            self.consume(TokenKind::Arrow, "expected '=>' in match arm")?;
            let body = self.parse_statement()?;
            let mut arm_span = match_pattern_span(&pattern);
            arm_span = span_union(arm_span, statement_span(&body));
            arms.push(MatchArm {
                pattern,
                body,
                span: arm_span,
            });

            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }

        let close_brace = self
            .consume(TokenKind::RBrace, "expected '}' to close match")?
            .clone();

        let mut span = span_union(keyword_span, expr_span(&expression));
        span = span_union(span, open_brace.span);
        span = span_union(span, close_brace.span);
        for arm in &arms {
            span = span_union(span, arm.span);
        }

        Some(Stmt::Match {
            expression,
            arms,
            span,
        })
    }

    fn parse_match_pattern(&mut self) -> Option<MatchPattern> {
        if self.match_token(TokenKind::Integer) {
            let token = self.previous().clone();
            let value = token
                .lexeme
                .parse::<i64>()
                .map_err(|_| {
                    self.errors.push(ParseError::new(
                        "invalid integer literal in match pattern",
                        token.span.start_location,
                    ));
                })
                .ok()?;
            return Some(MatchPattern::Literal {
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
                        "invalid float literal in match pattern",
                        token.span.start_location,
                    ));
                })
                .ok()?;
            return Some(MatchPattern::Literal {
                value: Literal::Float(value),
                span: token.span,
            });
        }

        if self.match_token(TokenKind::String) {
            let token = self.previous().clone();
            return Some(MatchPattern::Literal {
                value: Literal::String(token.lexeme.clone()),
                span: token.span,
            });
        }

        if self.match_keyword(Keyword::True) {
            let token = self.previous().clone();
            return Some(MatchPattern::Literal {
                value: Literal::Bool(true),
                span: token.span,
            });
        }

        if self.match_keyword(Keyword::False) {
            let token = self.previous().clone();
            return Some(MatchPattern::Literal {
                value: Literal::Bool(false),
                span: token.span,
            });
        }

        if self.match_token(TokenKind::Identifier) {
            let token = self.previous().clone();
            if token.lexeme != "_" {
                self.errors.push(ParseError::new(
                    "only '_' wildcard identifier is supported in match patterns",
                    token.span.start_location,
                ));
                return None;
            }
            return Some(MatchPattern::Identifier {
                name: token.lexeme,
                span: token.span,
            });
        }

        let location = self.peek().span.start_location;
        self.errors
            .push(ParseError::new("expected match pattern", location));
        None
    }

    fn parse_assignment_statement(&mut self) -> Option<Stmt> {
        let name_token = self
            .consume_identifier("expected identifier before assignment")?
            .clone();
        let equal = self
            .consume(TokenKind::Equal, "expected '=' in assignment")?
            .clone();
        let value = self.expression()?;
        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after assignment")?
            .clone();
        let span = span_union(name_token.span, semicolon.span);
        let span = span_union(span, expr_span(&value));
        let span = span_union(span, equal.span);
        Some(Stmt::Assignment {
            target: name_token.lexeme,
            value,
            span,
        })
    }

    fn parse_import(&mut self, import_token: Token) -> Option<Import> {
        let first = self
            .consume_identifier("expected module path after 'import'")?
            .clone();
        let mut segments = vec![first.lexeme.clone()];
        let mut span = span_union(import_token.span, first.span);

        while self.check(TokenKind::Dot) || self.check(TokenKind::Scope) {
            self.advance();
            let ident = self
                .consume_identifier("expected identifier in import path")?
                .clone();
            span = span_union(span, ident.span);
            segments.push(ident.lexeme.clone());
        }

        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after import statement")?
            .clone();
        span = span_union(span, semicolon.span);

        Some(Import {
            path: ModulePath { segments, span },
            span,
        })
    }

    fn parse_export(&mut self, export_token: Token) -> Option<Export> {
        let first = self
            .consume_identifier("expected module path after 'export'")?
            .clone();
        let mut identifiers = vec![first.clone()];
        let mut total_span = span_union(export_token.span, first.span);

        while self.check(TokenKind::Dot) || self.check(TokenKind::Scope) {
            self.advance();
            let ident = self
                .consume_identifier("expected identifier in export path")?
                .clone();
            total_span = span_union(total_span, ident.span);
            identifiers.push(ident);
        }

        if identifiers.len() < 2 {
            self.errors.push(ParseError::new(
                "export path must include a module and symbol (e.g., module.path::name)",
                identifiers[0].span.start_location,
            ));
            return None;
        }

        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after export statement")?
            .clone();
        total_span = span_union(total_span, semicolon.span);

        let symbol_token = identifiers.pop().expect("export path length verified");
        let module_span = identifiers
            .iter()
            .skip(1)
            .fold(identifiers[0].span, |acc, token| {
                span_union(acc, token.span)
            });
        let module_path = ModulePath {
            segments: identifiers
                .iter()
                .map(|token| token.lexeme.clone())
                .collect(),
            span: module_span,
        };

        Some(Export {
            module_path,
            symbol: symbol_token.lexeme.clone(),
            symbol_span: symbol_token.span,
            span: total_span,
        })
    }

    fn parse_module_declaration(&mut self, module_token: Token) -> Option<ModulePath> {
        let first = self
            .consume_identifier("expected module name after 'module'")?
            .clone();
        let mut segments = vec![first.lexeme.clone()];
        let mut span = span_union(module_token.span, first.span);

        while self.check(TokenKind::Dot) || self.check(TokenKind::Scope) {
            self.advance();
            let ident = self
                .consume_identifier("expected identifier in module path")?
                .clone();
            span = span_union(span, ident.span);
            segments.push(ident.lexeme.clone());
        }

        let semicolon = self
            .consume(
                TokenKind::Semicolon,
                "expected ';' after module declaration",
            )?
            .clone();
        span = span_union(span, semicolon.span);

        Some(ModulePath { segments, span })
    }

    fn parse_function(
        &mut self,
        fn_token: Token,
        visibility: Visibility,
        leading_span: Option<Span>,
    ) -> Option<Function> {
        let name_token = self.consume_identifier("expected function name")?.clone();

        self.consume(TokenKind::LParen, "expected '(' after function name")?;
        let parameters = self.parse_parameter_list()?;
        let close_paren = self
            .consume(TokenKind::RParen, "expected ')' after parameter list")?
            .clone();

        let return_type = if self.match_token(TokenKind::Colon) {
            Some(self.parse_type_name()?)
        } else {
            None
        };

        let open_brace = self
            .consume(TokenKind::LBrace, "expected '{' before function body")?
            .clone();
        let body = self.parse_block_from_open(open_brace.clone())?;
        let mut span = span_union(fn_token.span, body.span);
        span = span_union(span, close_paren.span);
        if let Some(leading) = leading_span {
            span = span_union(leading, span);
        }

        Some(Function {
            name: name_token.lexeme.clone(),
            parameters,
            return_type,
            body,
            visibility,
            span,
        })
    }

    fn parse_struct(
        &mut self,
        struct_token: Token,
        visibility: Visibility,
        leading_span: Option<Span>,
    ) -> Option<StructDecl> {
        let name_token = self.consume_identifier("expected struct name")?.clone();

        self.consume(TokenKind::LBrace, "expected '{' after struct name")?;
        let mut fields = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.match_token(TokenKind::Semicolon) || self.match_token(TokenKind::Comma) {
                continue;
            }

            let field_name = self.consume_identifier("expected field name")?.clone();
            self.consume(TokenKind::Colon, "expected ':' after field name")?;
            let field_type = self.parse_type_name()?;
            let field_span = span_union(field_name.span, field_type.span);

            fields.push(StructField {
                name: field_name.lexeme.clone(),
                ty: field_type,
                span: field_span,
            });

            if !self.match_token(TokenKind::Comma) && !self.check(TokenKind::RBrace) {
                self.consume(TokenKind::Comma, "expected ',' or '}' after struct field")?;
            }
        }

        let close_brace = self
            .consume(TokenKind::RBrace, "expected '}' to close struct")?
            .clone();

        let mut span = span_union(struct_token.span, close_brace.span);
        if let Some(leading) = leading_span {
            span = span_union(leading, span);
        }

        Some(StructDecl {
            name: name_token.lexeme.clone(),
            fields,
            visibility,
            span,
        })
    }

    fn parse_enum(
        &mut self,
        enum_token: Token,
        visibility: Visibility,
        leading_span: Option<Span>,
    ) -> Option<EnumDecl> {
        let name_token = self.consume_identifier("expected enum name")?.clone();

        self.consume(TokenKind::LBrace, "expected '{' after enum name")?;
        let mut variants = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.match_token(TokenKind::Semicolon) || self.match_token(TokenKind::Comma) {
                continue;
            }

            let variant_name = self.consume_identifier("expected variant name")?.clone();
            let variant_start = variant_name.span;

            // Check for variant data
            let data = if self.match_token(TokenKind::LParen) {
                // Tuple variant: Color(i32, i32, i32)
                let mut types = Vec::new();
                
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    types.push(self.parse_type_name()?);
                    
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
                
                self.consume(TokenKind::RParen, "expected ')' after tuple variant")?;
                Some(EnumVariantData::Tuple(types))
            } else if self.match_token(TokenKind::LBrace) {
                // Struct variant: Person { name: String, age: i32 }
                let mut fields = Vec::new();
                
                while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    if self.match_token(TokenKind::Semicolon) || self.match_token(TokenKind::Comma) {
                        continue;
                    }
                    
                    let field_name = self.consume_identifier("expected field name")?.clone();
                    self.consume(TokenKind::Colon, "expected ':' after field name")?;
                    let field_type = self.parse_type_name()?;
                    let field_span = span_union(field_name.span, field_type.span);
                    
                    fields.push(StructField {
                        name: field_name.lexeme.clone(),
                        ty: field_type,
                        span: field_span,
                    });
                    
                    if !self.match_token(TokenKind::Comma) && !self.check(TokenKind::RBrace) {
                        self.consume(TokenKind::Comma, "expected ',' or '}' after variant field")?;
                    }
                }
                
                self.consume(TokenKind::RBrace, "expected '}' to close struct variant")?;
                Some(EnumVariantData::Struct(fields))
            } else {
                // Unit variant: None
                None
            };

            let variant_span = if let Some(ref data) = data {
                match data {
                    EnumVariantData::Tuple(_) => span_union(variant_start, self.previous().span),
                    EnumVariantData::Struct(_) => span_union(variant_start, self.previous().span),
                }
            } else {
                variant_start
            };

            variants.push(EnumVariant {
                name: variant_name.lexeme.clone(),
                data,
                span: variant_span,
            });

            if !self.match_token(TokenKind::Comma) && !self.check(TokenKind::RBrace) {
                self.consume(TokenKind::Comma, "expected ',' or '}' after enum variant")?;
            }
        }

        let close_brace = self
            .consume(TokenKind::RBrace, "expected '}' to close enum")?
            .clone();

        let mut span = span_union(enum_token.span, close_brace.span);
        if let Some(leading) = leading_span {
            span = span_union(leading, span);
        }

        Some(EnumDecl {
            name: name_token.lexeme.clone(),
            variants,
            visibility,
            span,
        })
    }

    fn try_parse_visibility(&mut self) -> Option<(Visibility, Span)> {
        if self.check_keyword(Keyword::Pub) {
            let token = self.advance().clone();
            Some((Visibility::Public, token.span))
        } else {
            None
        }
    }

    fn parse_parameter_list(&mut self) -> Option<Vec<Parameter>> {
        let mut params = Vec::new();
        if self.check(TokenKind::RParen) {
            return Some(params);
        }

        loop {
            params.push(self.parse_parameter()?);
            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }

        Some(params)
    }

    fn parse_parameter(&mut self) -> Option<Parameter> {
        let name_token = self.consume_identifier("expected parameter name")?.clone();
        self.consume(TokenKind::Colon, "expected ':' after parameter name")?;
        let ty = self.parse_type_name()?;
        let span = span_union(name_token.span, ty.span);
        Some(Parameter {
            name: name_token.lexeme.clone(),
            ty,
            span,
        })
    }

    fn parse_block_from_open(&mut self, open: Token) -> Option<Block> {
        let mut statements = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            if self.match_token(TokenKind::Semicolon) {
                continue;
            }

            match self.parse_statement() {
                Some(stmt) => statements.push(stmt),
                None => self.synchronize(),
            }
        }

        let close = self
            .consume(TokenKind::RBrace, "expected '}' to close block")?
            .clone();
        let span = span_union(open.span, close.span);
        Some(Block { statements, span })
    }

    fn parse_constant(
        &mut self,
        keyword: Token,
        visibility: Visibility,
        visibility_span: Span,
    ) -> Option<Constant> {
        let (name, value, binding_span) = self.parse_binding_parts(keyword.span)?;
        let span = span_union(visibility_span, binding_span);
        Some(Constant {
            name,
            value,
            mutable: matches!(keyword.kind, TokenKind::Keyword(Keyword::Var)),
            visibility,
            span,
        })
    }

    fn parse_binding(&mut self, mutable: bool, keyword_span: Span) -> Option<Stmt> {
        let (name, value, span) = self.parse_binding_parts(keyword_span)?;
        Some(Stmt::Let {
            mutable,
            name,
            value,
            span,
        })
    }

    fn parse_binding_parts(&mut self, keyword_span: Span) -> Option<(String, Expr, Span)> {
        let name_token = self
            .consume_identifier("expected identifier after binding keyword")?
            .clone();
        self.consume(TokenKind::Equal, "expected '=' after identifier")?;
        let value = self.expression()?;
        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after binding")?
            .clone();
        let span = span_union(keyword_span, semicolon.span);
        Some((name_token.lexeme.clone(), value, span))
    }

    fn parse_expression_statement(&mut self) -> Option<Stmt> {
        let expr = self.expression()?;

        // Check if this is a field assignment: obj.field = value;
        if let Expr::FieldAccess { object, field, .. } = &expr {
            if self.match_token(TokenKind::Equal) {
                let value = self.expression()?;
                self.consume(TokenKind::Semicolon, "expected ';' after field assignment")?;
                let span = span_union(expr_span(&expr), self.previous().span);
                return Some(Stmt::FieldAssignment {
                    object: *object.clone(),
                    field: field.clone(),
                    value,
                    span,
                });
            }
        }

        self.consume(
            TokenKind::Semicolon,
            "expected ';' after expression statement",
        )?;
        Some(Stmt::Expr(expr))
    }

    fn parse_return(&mut self, keyword_span: Span) -> Option<Stmt> {
        if self.check(TokenKind::Semicolon) {
            let semicolon = self
                .consume(TokenKind::Semicolon, "expected ';' after return")?
                .clone();
            let span = span_union(keyword_span, semicolon.span);
            return Some(Stmt::Return { value: None, span });
        }

        let value = self.expression()?;
        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after return value")?
            .clone();
        let span = span_union(keyword_span, semicolon.span);
        Some(Stmt::Return {
            value: Some(value),
            span,
        })
    }

    fn parse_type_name(&mut self) -> Option<TypeName> {
        let first = self.consume_identifier("expected type name")?.clone();
        let mut segments = vec![first.lexeme.clone()];
        let mut span = first.span;

        while self.check(TokenKind::Dot) || self.check(TokenKind::Scope) {
            self.advance();
            let ident = self
                .consume_identifier("expected identifier in type path")?
                .clone();
            span = span_union(span, ident.span);
            segments.push(ident.lexeme.clone());
        }

        Some(TypeName { segments, span })
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.peek().kind, TokenKind::Keyword(current) if current == keyword)
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
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(TokenKind::LParen) {
                let call = self.finish_call(expr)?;
                expr = call;
            } else if self.match_token(TokenKind::Dot) {
                let field_token = self
                    .consume_identifier("expected field name after '.'")?
                    .clone();
                let span = span_union(expr_span(&expr), field_token.span);
                expr = Expr::FieldAccess {
                    object: Box::new(expr),
                    field: field_token.lexeme.clone(),
                    span,
                };
            } else if self.match_token(TokenKind::LBracket) {
                let index = self.expression()?;
                self.consume(TokenKind::RBracket, "expected ']' after array index")?;
                let span = span_union(expr_span(&expr), self.previous().span);
                expr = Expr::Index {
                    array: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else {
                break;
            }
        }

        Some(expr)
    }

    fn finish_call(&mut self, callee: Expr) -> Option<Expr> {
        let mut arguments = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                arguments.push(self.expression()?);
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }

        let close = self
            .consume(TokenKind::RParen, "expected ')' after arguments")?
            .clone();
        let span = span_union(expr_span(&callee), close.span);

        Some(Expr::Call {
            callee: Box::new(callee),
            arguments,
            span,
        })
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
            // Check if this is a struct literal
            if self.check(TokenKind::LBrace) {
                return self.parse_struct_literal(token);
            }
            return Some(Expr::Identifier {
                name: token.lexeme.clone(),
                span: token.span,
            });
        }

        if self.match_token(TokenKind::LBracket) {
            return self.parse_array_literal();
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

    fn peek_next_kind(&self) -> TokenKind {
        if self.current + 1 >= self.tokens.len() {
            TokenKind::Eof
        } else {
            self.tokens[self.current + 1].kind
        }
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

    fn parse_struct_literal(&mut self, type_token: Token) -> Option<Expr> {
        let start_span = type_token.span;
        self.consume(TokenKind::LBrace, "expected '{' for struct literal")?;

        let mut fields = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let field_token = self.consume_identifier("expected field name")?;
            let field_name = field_token.lexeme.clone();
            let field_start = field_token.span;

            self.consume(TokenKind::Colon, "expected ':' after field name")?;
            let value = self.expression()?;
            let field_span = span_union(field_start, expr_span(&value));

            fields.push(StructFieldInit {
                name: field_name,
                value,
                span: field_span,
            });

            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }

        self.consume(TokenKind::RBrace, "expected '}' after struct fields")?;
        let span = span_union(start_span, self.previous().span);

        Some(Expr::StructLiteral {
            name: type_token.lexeme.clone(),
            fields,
            span,
        })
    }

    fn parse_array_literal(&mut self) -> Option<Expr> {
        let start_span = self.previous().span;
        let mut elements = Vec::new();

        while !self.check(TokenKind::RBracket) && !self.is_at_end() {
            elements.push(self.expression()?);

            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }

        self.consume(TokenKind::RBracket, "expected ']' after array elements")?;
        let span = span_union(start_span, self.previous().span);

        Some(Expr::ArrayLiteral { elements, span })
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
                | TokenKind::Keyword(Keyword::Pub)
                | TokenKind::Keyword(Keyword::Export)
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
        | Expr::Call { span, .. }
        | Expr::Unary { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Grouping { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::StructLiteral { span, .. }
        | Expr::ArrayLiteral { span, .. }
        | Expr::Index { span, .. } => *span,
    }
}

fn statement_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Let { span, .. }
        | Stmt::Assignment { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::If { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. }
        | Stmt::FieldAssignment { span, .. }
        | Stmt::Block(Block { span, .. }) => *span,
        Stmt::Expr(expr) => expr_span(expr),
    }
}

fn match_pattern_span(pattern: &MatchPattern) -> Span {
    match pattern {
        MatchPattern::Literal { span, .. } | MatchPattern::Identifier { span, .. } => *span,
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
        assert_eq!(module.items.len(), 2);
    }

    #[test]
    fn parse_boolean_and_grouping() {
        let module = parse_ok("let flag = true; (flag);");
        assert_eq!(module.items.len(), 2);
    }

    #[test]
    fn parse_module_and_function() {
        let module = parse_ok("module demo.core; fn main(): void { return; }");
        assert!(module.name.is_some());
        assert_eq!(module.items.len(), 1);
        match &module.items[0] {
            Item::Function(func) => {
                assert_eq!(func.name, "main");
                assert!(func.return_type.is_some());
                assert_eq!(func.visibility, Visibility::Private);
            }
            _ => panic!("expected function item"),
        }
    }

    #[test]
    fn parse_public_function() {
        let module = parse_ok("pub fn exported(): void { return; }");
        assert_eq!(module.items.len(), 1);
        match &module.items[0] {
            Item::Function(func) => {
                assert_eq!(func.name, "exported");
                assert_eq!(func.visibility, Visibility::Public);
            }
            _ => panic!("expected function item"),
        }
    }

    #[test]
    fn parse_public_constant() {
        let module = parse_ok("pub let ANSWER = 42;");
        assert_eq!(module.items.len(), 1);
        match &module.items[0] {
            Item::Constant(constant) => {
                assert_eq!(constant.name, "ANSWER");
                assert!(!constant.mutable);
                assert_eq!(constant.visibility, Visibility::Public);
            }
            _ => panic!("expected constant item"),
        }
    }

    #[test]
    fn parse_import_statement() {
        let module = parse_ok("import demo.core; import utils.math; fn main() { return; }");
        assert_eq!(module.items.len(), 3);
        assert!(matches!(&module.items[0], Item::Import(_)));
        assert!(matches!(&module.items[1], Item::Import(_)));
    }

    #[test]
    fn parse_export_statement() {
        let module = parse_ok("export demo.core::value; fn main() { return; }");
        assert!(matches!(&module.items[0], Item::Export(_)));
    }

    #[test]
    fn parse_function_call_expression() {
        let module = parse_ok("fn main(): i32 { return add(1, 2); }");
        match &module.items[0] {
            Item::Function(Function { body, .. }) => match &body.statements[0] {
                Stmt::Return {
                    value: Some(expr), ..
                } => match expr {
                    Expr::Call { arguments, .. } => {
                        assert_eq!(arguments.len(), 2);
                    }
                    _ => panic!("expected call expression"),
                },
                _ => panic!("expected return statement"),
            },
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_if_else_statement() {
        let module = parse_ok("fn main() { if (true) { return; } else { return; } }");
        match &module.items[0] {
            Item::Function(Function { body, .. }) => match &body.statements[0] {
                Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    assert!(matches!(
                        condition,
                        Expr::Literal {
                            value: Literal::Bool(true),
                            ..
                        }
                    ));
                    assert!(matches!(**then_branch, Stmt::Block(_)));
                    assert!(else_branch.is_some());
                }
                _ => panic!("expected if statement"),
            },
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_while_statement_with_assignment() {
        let module = parse_ok("fn main() { var x = 0; while (x < 10) { x = x + 1; } }");
        match &module.items[0] {
            Item::Function(Function { body, .. }) => {
                assert!(matches!(body.statements[0], Stmt::Let { .. }));
                match &body.statements[1] {
                    Stmt::While {
                        body: loop_body, ..
                    } => match &**loop_body {
                        Stmt::Block(Block { statements, .. }) => {
                            assert!(matches!(statements[0], Stmt::Assignment { .. }));
                        }
                        _ => panic!("expected loop block"),
                    },
                    _ => panic!("expected while statement"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_break_and_continue_in_loop() {
        let module = parse_ok("fn main() { while (true) { break; continue; } return; }");
        match &module.items[0] {
            Item::Function(Function { body, .. }) => match &body.statements[0] {
                Stmt::While {
                    body: loop_body, ..
                } => match &**loop_body {
                    Stmt::Block(Block { statements, .. }) => {
                        assert!(matches!(statements[0], Stmt::Break { .. }));
                        assert!(matches!(statements[1], Stmt::Continue { .. }));
                    }
                    _ => panic!("expected block"),
                },
                _ => panic!("expected while statement"),
            },
            _ => panic!("expected function"),
        }
    }
}
