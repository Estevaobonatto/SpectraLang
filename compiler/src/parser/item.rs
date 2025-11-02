use crate::{
    ast::{
        Block, Function, FunctionParam, ImplBlock, Item, Method, Parameter, TraitDeclaration,
        TraitMethod, Visibility,
    },
    span::{span_union, Span},
    token::{Keyword, TokenKind},
};

use super::Parser;

impl Parser {
    pub(super) fn parse_item(&mut self) -> Result<Item, ()> {
        match &self.current().kind {
            crate::token::TokenKind::Keyword(Keyword::Import) => {
                let import = self.parse_import()?;
                Ok(Item::Import(import))
            }
            crate::token::TokenKind::Keyword(Keyword::Pub) => {
                self.advance(); // consume 'pub'
                self.parse_item_with_visibility(Visibility::Public)
            }
            crate::token::TokenKind::Keyword(Keyword::Fn) => {
                self.parse_item_with_visibility(Visibility::Private)
            }
            crate::token::TokenKind::Keyword(Keyword::Struct) => {
                self.parse_item_with_visibility(Visibility::Private)
            }
            crate::token::TokenKind::Keyword(Keyword::Enum) => {
                self.parse_item_with_visibility(Visibility::Private)
            }
            crate::token::TokenKind::Keyword(Keyword::Impl) => {
                let impl_block = self.parse_impl_block()?;
                Ok(Item::Impl(impl_block))
            }
            crate::token::TokenKind::Keyword(Keyword::Trait) => {
                let trait_decl = self.parse_trait_declaration()?;
                Ok(Item::Trait(trait_decl))
            }
            _ => {
                self.error("Expected item declaration (import, fn, etc.)");
                Err(())
            }
        }
    }

    fn parse_item_with_visibility(&mut self, visibility: Visibility) -> Result<Item, ()> {
        match &self.current().kind {
            crate::token::TokenKind::Keyword(Keyword::Fn) => {
                let function = self.parse_function(visibility)?;
                Ok(Item::Function(function))
            }
            crate::token::TokenKind::Keyword(Keyword::Struct) => {
                let struct_item = self.parse_struct(visibility)?;
                Ok(Item::Struct(struct_item))
            }
            crate::token::TokenKind::Keyword(Keyword::Enum) => {
                let enum_item = self.parse_enum(visibility)?;
                Ok(Item::Enum(enum_item))
            }
            crate::token::TokenKind::Keyword(Keyword::Impl) => {
                let impl_block = self.parse_impl_block()?;
                Ok(Item::Impl(impl_block))
            }
            _ => {
                self.error("Expected function, struct, enum, or impl declaration");
                Err(())
            }
        }
    }

    pub(super) fn parse_function(&mut self, visibility: Visibility) -> Result<Function, ()> {
        // Expect: fn <name>(<params>) [-> type] { <body> }
        let start_span = self.consume_keyword(Keyword::Fn, "Expected 'fn' keyword")?;

        let (name, _name_span) = self.consume_identifier("Expected function name")?;

        self.consume_symbol('(', "Expected '(' after function name")?;

        let params = self.parse_function_params()?;

        self.consume_symbol(')', "Expected ')' after function parameters")?;

        // Optional return type
        let return_type = if matches!(
            &self.current().kind,
            crate::token::TokenKind::Operator(crate::token::Operator::Arrow)
        ) {
            self.advance(); // consume '->'
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let end_span = body.span;

        Ok(Function {
            name,
            span: span_union(start_span, end_span),
            visibility,
            params,
            return_type,
            body,
        })
    }

    fn parse_function_params(&mut self) -> Result<Vec<FunctionParam>, ()> {
        let mut params = Vec::new();

        // Check for empty parameter list
        if self.check_symbol(')') {
            return Ok(params);
        }

        loop {
            let (name, name_span) = self.consume_identifier("Expected parameter name")?;

            // Optional type annotation
            let ty = if self.check_symbol(':') {
                self.advance(); // consume ':'
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            params.push(FunctionParam {
                name,
                span: name_span,
                ty,
            });

            if !self.check_symbol(',') {
                break;
            }
            self.advance(); // consume ','
        }

        Ok(params)
    }

    pub(super) fn parse_block(&mut self) -> Result<Block, ()> {
        let start_span = self.consume_symbol('{', "Expected '{' to start block")?;

        let mut statements = Vec::new();

        while !self.check_symbol('}') && !self.is_at_end() {
            // Try to parse as statement
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(_) => {
                    // If parsing failed and next is '}', try parsing as final expression without ';'
                    if self.check_symbol('}') {
                        break; // Let the error propagate, block might be empty
                    }
                    self.synchronize();
                }
            }
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end block")?;

        Ok(Block {
            span: span_union(start_span, end_span),
            statements,
        })
    }

    pub(super) fn parse_struct(
        &mut self,
        visibility: Visibility,
    ) -> Result<crate::ast::Struct, ()> {
        use crate::ast::{Struct, StructField};

        // Expect: struct <name> { <fields> }
        let start_span = self.consume_keyword(Keyword::Struct, "Expected 'struct' keyword")?;

        let (name, _name_span) = self.consume_identifier("Expected struct name")?;

        self.consume_symbol('{', "Expected '{' after struct name")?;

        let mut fields = Vec::new();

        // Parse fields
        while !self.check_symbol('}') && !self.is_at_end() {
            // Parse field: <name>: <type>
            let (field_name, field_span) = self.consume_identifier("Expected field name")?;

            self.consume_symbol(':', "Expected ':' after field name")?;

            let field_type = self.parse_type_annotation()?;

            fields.push(StructField {
                name: field_name,
                span: field_span,
                ty: field_type,
            });

            // Optional comma
            if self.check_symbol(',') {
                self.advance();
            }
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end struct")?;

        Ok(Struct {
            name,
            span: span_union(start_span, end_span),
            visibility,
            fields,
        })
    }

    pub(super) fn parse_enum(&mut self, visibility: Visibility) -> Result<crate::ast::Enum, ()> {
        use crate::ast::{Enum, EnumVariant};

        // Expect: enum <name> { <variants> }
        let start_span = self.consume_keyword(Keyword::Enum, "Expected 'enum' keyword")?;

        let (name, _name_span) = self.consume_identifier("Expected enum name")?;

        self.consume_symbol('{', "Expected '{' after enum name")?;

        let mut variants = Vec::new();

        // Parse variants
        while !self.check_symbol('}') && !self.is_at_end() {
            // Parse variant: <name> or <name>(<types>)
            let (variant_name, variant_span) = self.consume_identifier("Expected variant name")?;

            let data = if self.check_symbol('(') {
                self.advance(); // consume '('

                let mut types = Vec::new();

                // Parse tuple variant data types
                if !self.check_symbol(')') {
                    loop {
                        types.push(self.parse_type_annotation()?);
                        if !self.check_symbol(',') {
                            break;
                        }
                        self.advance(); // consume ','
                    }
                }

                self.consume_symbol(')', "Expected ')' after variant data")?;
                Some(types)
            } else {
                None // Unit variant
            };

            variants.push(EnumVariant {
                name: variant_name,
                span: variant_span,
                data,
            });

            // Optional comma
            if self.check_symbol(',') {
                self.advance();
            }
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end enum")?;

        Ok(Enum {
            name,
            span: span_union(start_span, end_span),
            visibility,
            variants,
        })
    }

    pub(super) fn parse_impl_block(&mut self) -> Result<ImplBlock, ()> {
        // Expect: impl TypeName { methods... } ou impl TraitName for TypeName { methods... }
        let start_span = self.consume_keyword(Keyword::Impl, "Expected 'impl' keyword")?;

        let (first_name, _) =
            self.consume_identifier("Expected trait or type name after 'impl'")?;

        // Checar se é "impl Trait for Type" ou "impl Type"
        if self.check_keyword(Keyword::For) {
            // É um trait impl: impl TraitName for TypeName
            self.advance(); // consume 'for'
            let (type_name, _) = self.consume_identifier("Expected type name after 'for'")?;
            return self.parse_trait_impl_block(start_span, first_name, type_name);
        }

        // É um impl regular: impl TypeName
        let type_name = first_name;

        self.consume_symbol('{', "Expected '{' to start impl block")?;

        let mut methods = Vec::new();

        while !self.check_symbol('}') && !self.is_at_end() {
            // Parse method: fn method_name(params) -> type { body }
            self.consume_keyword(Keyword::Fn, "Expected 'fn' keyword for method")?;

            let (method_name, method_name_span) =
                self.consume_identifier("Expected method name")?;

            self.consume_symbol('(', "Expected '(' after method name")?;

            // Parse parameters (pode incluir self)
            let mut params = Vec::new();

            while !self.check_symbol(')') && !self.is_at_end() {
                let param_start = self.current().span;

                // Verificar se é self, &self, ou &mut self
                let (is_self, is_reference, is_mutable) = if self.check_keyword(Keyword::Mut) {
                    self.advance(); // consume 'mut'
                    if self.check_identifier() {
                        if let TokenKind::Identifier(name) = &self.current().kind {
                            if name == "self" {
                                self.advance();
                                (true, false, true) // mut self
                            } else {
                                (false, false, true) // parâmetro mut normal
                            }
                        } else {
                            (false, false, true)
                        }
                    } else {
                        (false, false, true)
                    }
                } else if self.check_symbol('&') {
                    self.advance(); // consume '&'
                    if self.check_keyword(Keyword::Mut) {
                        self.advance(); // consume 'mut'
                        self.consume_identifier("Expected 'self' after '&mut'")?;
                        (true, true, true) // &mut self
                    } else {
                        self.consume_identifier("Expected 'self' after '&'")?;
                        (true, true, false) // &self
                    }
                } else if self.check_identifier() {
                    if let TokenKind::Identifier(name) = &self.current().kind {
                        if name == "self" {
                            self.advance();
                            (true, false, false) // self
                        } else {
                            (false, false, false) // parâmetro normal
                        }
                    } else {
                        (false, false, false)
                    }
                } else {
                    (false, false, false) // parâmetro normal
                };

                let (param_name, type_annotation) = if is_self {
                    ("self".to_string(), None)
                } else {
                    let (name, _) = self.consume_identifier("Expected parameter name")?;
                    self.consume_symbol(':', "Expected ':' after parameter name")?;
                    let ty = self.parse_type_annotation()?;
                    (name, Some(ty))
                };

                let param_end = self.current().span;
                params.push(Parameter {
                    name: param_name,
                    type_annotation,
                    is_self,
                    is_reference,
                    is_mutable,
                    span: span_union(param_start, param_end),
                });

                // Optional comma
                if self.check_symbol(',') {
                    self.advance();
                }
            }

            self.consume_symbol(')', "Expected ')' after method parameters")?;

            // Optional return type
            let return_type = if matches!(
                &self.current().kind,
                crate::token::TokenKind::Operator(crate::token::Operator::Arrow)
            ) {
                self.advance(); // consume '->'
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            let body = self.parse_block()?;
            let body_end_span = body.span;

            methods.push(Method {
                name: method_name,
                params,
                return_type,
                body,
                span: span_union(method_name_span, body_end_span),
            });
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end impl block")?;

        Ok(ImplBlock {
            type_name,
            trait_name: None, // impl regular
            methods,
            span: span_union(start_span, end_span),
        })
    }

    /// Parse trait declaration: trait Name { fn method(&self) -> Type; }
    pub(super) fn parse_trait_declaration(&mut self) -> Result<TraitDeclaration, ()> {
        // Expect: trait <name> { <method signatures> }
        let start_span = self.consume_keyword(Keyword::Trait, "Expected 'trait' keyword")?;

        let (name, _name_span) = self.consume_identifier("Expected trait name")?;

        self.consume_symbol('{', "Expected '{' after trait name")?;

        let mut methods = Vec::new();

        // Parse method signatures (sem corpo, apenas assinaturas)
        while !self.check_symbol('}') && !self.is_at_end() {
            let method_start =
                self.consume_keyword(Keyword::Fn, "Expected 'fn' for method signature")?;

            let (method_name, method_name_span) =
                self.consume_identifier("Expected method name")?;

            self.consume_symbol('(', "Expected '(' after method name")?;

            // Parse parameters (igual a métodos regulares)
            let mut params = Vec::new();

            while !self.check_symbol(')') && !self.is_at_end() {
                let param_start = self.current().span;

                // Check for self parameter
                let is_self_param =
                    matches!(&self.current().kind, TokenKind::Identifier(id) if id == "self");

                let (is_self, is_reference, is_mutable) = if is_self_param {
                    self.advance();
                    (true, false, false)
                } else if self.check_symbol('&') {
                    self.advance();
                    let is_mut = if self.check_keyword(Keyword::Mut) {
                        self.advance();
                        true
                    } else {
                        false
                    };
                    let is_self_after_ref =
                        matches!(&self.current().kind, TokenKind::Identifier(id) if id == "self");
                    if !is_self_after_ref {
                        self.error("Expected 'self' after '&'");
                        return Err(());
                    }
                    self.advance();
                    (true, true, is_mut)
                } else {
                    // Regular parameter
                    let (param_name, _) = self.consume_identifier("Expected parameter name")?;
                    self.consume_symbol(':', "Expected ':' after parameter name")?;
                    let param_type = self.parse_type_annotation()?;
                    let param_end = param_type.span;

                    params.push(Parameter {
                        name: param_name,
                        type_annotation: Some(param_type),
                        is_self: false,
                        is_reference: false,
                        is_mutable: false,
                        span: span_union(param_start, param_end),
                    });

                    if self.check_symbol(',') {
                        self.advance();
                    }
                    continue;
                };

                // Para self, o span vai até o token atual - 1
                // Como já avançamos, precisamos usar param_start até onde paramos
                let param_end = param_start; // Simplificado - usamos só o start
                params.push(Parameter {
                    name: "self".to_string(),
                    type_annotation: None,
                    is_self,
                    is_reference,
                    is_mutable,
                    span: param_start,
                });

                if self.check_symbol(',') {
                    self.advance();
                }
            }

            self.consume_symbol(')', "Expected ')' after method parameters")?;

            // Optional return type
            let return_type = if matches!(
                &self.current().kind,
                crate::token::TokenKind::Operator(crate::token::Operator::Arrow)
            ) {
                self.advance(); // consume '->'
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            // Trait methods não têm corpo, apenas assinatura
            let method_end =
                self.consume_symbol(';', "Expected ';' after trait method signature")?;

            methods.push(TraitMethod {
                name: method_name,
                params,
                return_type,
                span: span_union(method_start, method_end),
            });
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end trait declaration")?;

        Ok(TraitDeclaration {
            name,
            methods,
            span: span_union(start_span, end_span),
        })
    }

    /// Parse trait implementation: impl TraitName for TypeName { methods... }
    fn parse_trait_impl_block(
        &mut self,
        start_span: Span,
        _trait_name: String,
        type_name: String,
    ) -> Result<ImplBlock, ()> {
        // Nota: Por enquanto, vamos retornar um ImplBlock regular
        // TODO: Criar Item::TraitImpl separado e validar que métodos correspondem ao trait

        self.consume_symbol('{', "Expected '{' to start trait impl block")?;

        let mut methods = Vec::new();

        while !self.check_symbol('}') && !self.is_at_end() {
            // Parse method (igual ao impl block regular)
            let method_name_span = self.consume_keyword(Keyword::Fn, "Expected 'fn' for method")?;

            let (method_name, method_name_span) =
                self.consume_identifier("Expected method name")?;

            self.consume_symbol('(', "Expected '(' after method name")?;

            // Parse parameters
            let mut params = Vec::new();

            while !self.check_symbol(')') && !self.is_at_end() {
                let param_start = self.current().span;

                // Check for self parameter
                let is_self_param =
                    matches!(&self.current().kind, TokenKind::Identifier(id) if id == "self");

                let (is_self, is_reference, is_mutable) = if is_self_param {
                    self.advance();
                    (true, false, false)
                } else if self.check_symbol('&') {
                    self.advance();
                    let is_mut = if self.check_keyword(Keyword::Mut) {
                        self.advance();
                        true
                    } else {
                        false
                    };
                    let is_self_after_ref =
                        matches!(&self.current().kind, TokenKind::Identifier(id) if id == "self");
                    if !is_self_after_ref {
                        self.error("Expected 'self' after '&'");
                        return Err(());
                    }
                    self.advance();
                    (true, true, is_mut)
                } else {
                    // Regular parameter
                    let (param_name, _) = self.consume_identifier("Expected parameter name")?;
                    self.consume_symbol(':', "Expected ':' after parameter name")?;
                    let param_type = self.parse_type_annotation()?;
                    let param_end = param_type.span;

                    params.push(Parameter {
                        name: param_name,
                        type_annotation: Some(param_type),
                        is_self: false,
                        is_reference: false,
                        is_mutable: false,
                        span: span_union(param_start, param_end),
                    });

                    if self.check_symbol(',') {
                        self.advance();
                    }
                    continue;
                };

                params.push(Parameter {
                    name: "self".to_string(),
                    type_annotation: None,
                    is_self,
                    is_reference,
                    is_mutable,
                    span: param_start,
                });

                if self.check_symbol(',') {
                    self.advance();
                }
            }

            self.consume_symbol(')', "Expected ')' after method parameters")?;

            // Optional return type
            let return_type = if matches!(
                &self.current().kind,
                crate::token::TokenKind::Operator(crate::token::Operator::Arrow)
            ) {
                self.advance(); // consume '->'
                Some(self.parse_type_annotation()?)
            } else {
                None
            };

            let body = self.parse_block()?;
            let body_end_span = body.span;

            methods.push(Method {
                name: method_name,
                params,
                return_type,
                body,
                span: span_union(method_name_span, body_end_span),
            });
        }

        let end_span = self.consume_symbol('}', "Expected '}' to end trait impl block")?;

        // Retorna ImplBlock com trait_name preenchido
        Ok(ImplBlock {
            type_name,
            trait_name: Some(_trait_name), // impl Trait for Type
            methods,
            span: span_union(start_span, end_span),
        })
    }
}
