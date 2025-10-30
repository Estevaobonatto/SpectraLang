use crate::{
    ast::{
        BinaryOperator, Block, Expr, Function, Import, Item, Literal, Module, ModulePath,
        Parameter, Stmt, TypeName, UnaryOperator,
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
                match self.parse_function(fn_token) {
                    Some(function) => items.push(Item::Function(function)),
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
        self.parse_equality()
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
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

        self.parse_expression_statement()
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

    fn parse_function(&mut self, fn_token: Token) -> Option<Function> {
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
        let span = span_union(fn_token.span, body.span);

        Some(Function {
            name: name_token.lexeme.clone(),
            parameters,
            return_type,
            body,
            span: span_union(span, close_paren.span),
        })
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

    fn parse_binding(&mut self, mutable: bool, keyword_span: Span) -> Option<Stmt> {
        let name_token = self
            .consume_identifier("expected identifier after binding keyword")?
            .clone();
        self.consume(TokenKind::Equal, "expected '=' after identifier")?;
        let value = self.expression()?;
        let semicolon = self
            .consume(TokenKind::Semicolon, "expected ';' after binding")?
            .clone();
        let span = span_union(keyword_span, semicolon.span);

        Some(Stmt::Let {
            mutable,
            name: name_token.lexeme.clone(),
            value,
            span,
        })
    }

    fn parse_expression_statement(&mut self) -> Option<Stmt> {
        let expr = self.expression()?;
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
        | Expr::Call { span, .. }
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
            }
            _ => panic!("expected function item"),
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
}
