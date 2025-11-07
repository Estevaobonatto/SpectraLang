mod expression;
mod item;
mod module;
mod statement;
mod type_annotation;

use crate::{
    ast::{Module, TypeAnnotation, TypeAnnotationKind},
    error::ParseError,
    span::Span,
    token::{Keyword, Token, TokenKind},
};
use std::collections::HashMap;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    errors: Vec<ParseError>,
    trait_signatures: HashMap<String, HashMap<String, TraitMethodSignature>>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
            errors: Vec::new(),
            trait_signatures: HashMap::new(),
        }
    }

    pub fn parse(mut self) -> Result<Module, Vec<ParseError>> {
        let module = self.parse_module();

        if self.errors.is_empty() {
            Ok(module)
        } else {
            Err(self.errors)
        }
    }

    // === Token Navigation Methods ===

    fn current(&self) -> &Token {
        self.tokens
            .get(self.position)
            .unwrap_or_else(|| self.tokens.last().expect("tokens should not be empty"))
    }

    #[allow(dead_code)]
    fn peek(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.position + offset)
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.position += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current().kind, TokenKind::EndOfFile)
    }

    // === Token Checking Methods ===

    #[allow(dead_code)]
    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(&self.current().kind, TokenKind::Keyword(k) if *k == keyword)
    }

    fn check_symbol(&self, symbol: char) -> bool {
        matches!(&self.current().kind, TokenKind::Symbol(s) if *s == symbol)
    }

    #[allow(dead_code)]
    fn check_identifier(&self) -> bool {
        matches!(self.current().kind, TokenKind::Identifier(_))
    }

    // === Token Consumption Methods ===

    fn consume_keyword(&mut self, keyword: Keyword, error_message: &str) -> Result<Span, ()> {
        if self.check_keyword(keyword) {
            let span = self.current().span;
            self.advance();
            Ok(span)
        } else {
            let span = self.current().span;
            let context = format!(
                "expected keyword `{}`, found {}",
                keyword,
                Self::describe_token(self.current())
            );
            let hint = self.keyword_hint(keyword);
            self.push_error(error_message, span, hint, Some(context));
            Err(())
        }
    }

    fn consume_symbol(&mut self, symbol: char, error_message: &str) -> Result<Span, ()> {
        if self.check_symbol(symbol) {
            let span = self.current().span;
            self.advance();
            Ok(span)
        } else {
            let span = self.current().span;
            let context = format!(
                "expected symbol `{}`, found {}",
                symbol,
                Self::describe_token(self.current())
            );
            let hint = self.symbol_hint(symbol);
            self.push_error(error_message, span, hint, Some(context));
            Err(())
        }
    }

    fn consume_identifier(&mut self, error_message: &str) -> Result<(String, Span), ()> {
        if let TokenKind::Identifier(name) = &self.current().kind.clone() {
            let name = name.clone();
            let span = self.current().span;
            self.advance();
            Ok((name, span))
        } else {
            let span = self.current().span;
            let context = format!(
                "expected identifier, found {}",
                Self::describe_token(self.current())
            );
            self.push_error(error_message, span, None, Some(context));
            Err(())
        }
    }

    // === Error Handling ===

    fn error(&mut self, message: &str) {
        let span = self.current().span;
        self.push_error(message, span, None, None);
    }

    #[allow(dead_code)]
    fn error_at(&mut self, message: &str, span: Span) {
        self.push_error(message, span, None, None);
    }

    // === Synchronization ===

    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.check_symbol(';') {
                self.advance();
                return;
            }

            match &self.current().kind {
                TokenKind::Keyword(Keyword::Module)
                | TokenKind::Keyword(Keyword::Import)
                | TokenKind::Keyword(Keyword::Fn)
                | TokenKind::Keyword(Keyword::Class)
                | TokenKind::Keyword(Keyword::Trait)
                | TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::Return) => return,
                _ => {}
            }

            self.advance();
        }
    }

    fn push_error(
        &mut self,
        message: impl Into<String>,
        span: Span,
        hint: Option<String>,
        context: Option<String>,
    ) {
        let mut error = ParseError::new(message, span);

        if let Some(context) = context {
            error = error.with_context(context);
        } else {
            error = error.with_context(format!("found {}", Self::describe_token(self.current())));
        }

        if let Some(hint) = hint {
            error = error.with_hint(hint);
        }

        self.errors.push(error);
    }

    fn describe_token(token: &Token) -> String {
        match &token.kind {
            TokenKind::Identifier(name) => format!("identifier `{}`", name),
            TokenKind::Number(value) => format!("number `{}`", value),
            TokenKind::Keyword(keyword) => format!("keyword `{}`", keyword),
            TokenKind::Symbol(symbol) => format!("symbol `{}`", symbol),
            TokenKind::Operator(op) => format!("operator `{}`", op),
            TokenKind::StringLiteral(value) => {
                const MAX_PREVIEW: usize = 24;
                if value.len() > MAX_PREVIEW {
                    let mut preview = value[..MAX_PREVIEW].to_string();
                    preview.push_str("…");
                    format!("string literal \"{}\"", preview)
                } else {
                    format!("string literal \"{}\"", value)
                }
            }
            TokenKind::EndOfFile => "end of file".to_string(),
        }
    }

    fn keyword_hint(&self, keyword: Keyword) -> Option<String> {
        match keyword {
            Keyword::Module => Some("Start the file with `module <name>;`.".to_string()),
            Keyword::Import => Some("Use `import path.to.module;` to bring other modules into scope.".to_string()),
            Keyword::Fn => Some("Function declarations start with `fn name(...)`.".to_string()),
            Keyword::Trait => Some("Traits are declared with `trait TraitName { ... }`.".to_string()),
            Keyword::Impl => Some("Use `impl Type` to provide trait implementations.".to_string()),
            Keyword::Let => Some("Introduce bindings with `let name = expression;`.".to_string()),
            Keyword::Return => Some("Use `return expression;` to exit a function early.".to_string()),
            Keyword::While | Keyword::For | Keyword::Loop => {
                Some("Loops require a control keyword such as `while`, `for`, or `loop`.".to_string())
            }
            _ => None,
        }
    }

    fn symbol_hint(&self, symbol: char) -> Option<String> {
        match symbol {
            ';' => Some("Add a `;` to terminate the previous statement.".to_string()),
            ')' => Some("Close the parenthesis with `)`.".to_string()),
            '}' => Some("Close the block with `}`.".to_string()),
            ']' => Some("Close the bracket with `]`.".to_string()),
            '{' => Some("Insert `{` to open a block.".to_string()),
            '(' => Some("Insert `(` to start the parameter or argument list.".to_string()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitMethodSignature {
    pub params: Vec<ParameterSignature>,
    pub return_type: Option<TypePattern>,
    pub has_default_body: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterSignature {
    pub is_self: bool,
    pub is_reference: bool,
    pub is_mutable: bool,
    pub ty: Option<TypePattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypePattern {
    Simple(Vec<String>),
    Tuple(Vec<TypePattern>),
}

impl TypePattern {
    pub fn from_annotation(annotation: &TypeAnnotation) -> Self {
        match &annotation.kind {
            TypeAnnotationKind::Simple { segments } => TypePattern::Simple(segments.clone()),
            TypeAnnotationKind::Tuple { elements } => {
                TypePattern::Tuple(elements.iter().map(TypePattern::from_annotation).collect())
            }
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            TypePattern::Simple(segments) => segments.join("::"),
            TypePattern::Tuple(elements) => {
                let inner = elements
                    .iter()
                    .map(|elem| elem.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
        }
    }
}
