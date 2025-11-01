use crate::{
    ast::{Block, Function, FunctionParam, Item, Visibility},
    span::span_union,
    token::Keyword,
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
            _ => {
                self.error("Expected function, struct, or enum declaration");
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

    pub(super) fn parse_struct(&mut self, visibility: Visibility) -> Result<crate::ast::Struct, ()> {
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
}
