mod expression;
mod item;
mod module;
mod statement;
mod type_annotation;
pub mod workspace;

use crate::{
    ast::{Module, TypeAnnotation, TypeAnnotationKind},
    error::ParseError,
    span::{Span, Location},
    token::{Keyword, Token, TokenKind},
};
use std::collections::{HashMap, HashSet};

pub struct Parser {
    tokens: Vec<Token>,
    /// Sentinela EOF devolvido por `current()` quando position é maior que tokens.
    /// Garante que o parser nunca panique por lista de tokens vazia.
    eof_sentinel: Token,
    position: usize,
    errors: Vec<ParseError>,
    trait_signatures: HashMap<String, HashMap<String, TraitMethodSignature>>,
    enabled_features: HashSet<String>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, enabled_features: HashSet<String>) -> Self {
        let eof_sentinel = Token::new(
            TokenKind::EndOfFile,
            Span::new(0, 0, Location::new(1, 1), Location::new(1, 1)),
        );
        Self {
            tokens,
            eof_sentinel,
            position: 0,
            errors: Vec::new(),
            // Pre-allocate with a reasonable capacity to reduce rehashing while
            // parsing modules that typically have a handful of known traits.
            trait_signatures: HashMap::with_capacity(8),
            enabled_features,
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
            .unwrap_or(&self.eof_sentinel)
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

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(&self.current().kind, TokenKind::Keyword(k) if *k == keyword)
    }

    fn check_symbol(&self, symbol: char) -> bool {
        matches!(&self.current().kind, TokenKind::Symbol(s) if *s == symbol)
    }

    fn check_identifier(&self) -> bool {
        matches!(self.current().kind, TokenKind::Identifier(_))
    }

    // === Token Consumption Methods ===

    fn consume_keyword(&mut self, keyword: Keyword, error_message: &str) -> Result<Span, ()> {
        if self.check_keyword(keyword.clone()) {
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
            let hint = self.keyword_hint(keyword.clone());
            self.push_error(error_message, span, hint, Some(context));
            Err(())
        }
    }

    fn consume_symbol(&mut self, symbol: char, error_message: &str) -> Result<Span, ()> {
        if self.check_symbol(symbol) {
            let span = self.current().span;
            self.advance();
            Ok(span)
        } else if let Some(recovery_span) = self.recover_missing_symbol(symbol) {
            let context = format!(
                "missing `{}` before {}",
                symbol,
                Self::describe_token(self.current())
            );
            let hint = self
                .symbol_hint(symbol)
                .or_else(|| Some(format!("Insert `{}` here.", symbol)));
            self.push_error(error_message, recovery_span, hint, Some(context));
            Ok(recovery_span)
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

    fn error_at(&mut self, message: &str, span: Span) {
        self.push_error(message, span, None, None);
    }

    // === Synchronization ===

    fn synchronize(&mut self) {
        // Check if the current token is already a recovery boundary before
        // advancing, so we don't inadvertently skip valid constructs.
        if self.is_at_boundary() {
            // Consume a ';' boundary so the caller starts fresh after it.
            if self.check_symbol(';') {
                self.advance();
            }
            return;
        }

        self.advance();

        while !self.is_at_end() {
            if self.check_symbol(';') {
                self.advance();
                return;
            }

            if self.is_at_boundary() {
                return;
            }

            self.advance();
        }
    }

    /// Returns `true` when the current token is a natural recovery boundary
    /// (a `}` or a keyword that starts a new top-level / statement construct).
    fn is_at_boundary(&self) -> bool {
        if self.check_symbol('}') {
            return true;
        }
        matches!(
            &self.current().kind,
            TokenKind::Keyword(Keyword::Module)
                | TokenKind::Keyword(Keyword::Import)
                | TokenKind::Keyword(Keyword::Fn)
                | TokenKind::Keyword(Keyword::Class)
                | TokenKind::Keyword(Keyword::Trait)
                | TokenKind::Keyword(Keyword::Let)
                | TokenKind::Keyword(Keyword::Return)
                | TokenKind::Keyword(Keyword::Else)
                | TokenKind::Keyword(Keyword::Elif)
                | TokenKind::Keyword(Keyword::ElseIf)
                | TokenKind::Keyword(Keyword::Case)
                | TokenKind::Keyword(Keyword::Switch)
        )
    }

    fn is_feature_enabled(&self, feature: &str) -> bool {
        self.enabled_features.contains(feature)
    }

    fn require_feature(&mut self, feature: &str, span: Span, description: &str) -> Result<(), ()> {
        if self.is_feature_enabled(feature) {
            return Ok(());
        }

        let message = format!(
            "{} require enabling the experimental '{}' feature",
            description, feature
        );
        let hint = format!(
            "Re-run with --enable-experimental {} to opt into {}.",
            feature,
            description.to_lowercase()
        );
        let context = format!("feature '{}' is disabled for this compilation", feature);
        self.push_error(message, span, Some(hint), Some(context));
        Err(())
    }

    fn recover_missing_symbol(&self, symbol: char) -> Option<Span> {
        match symbol {
            ';' => {
                if self.is_at_end() || self.is_statement_boundary_token() {
                    return Some(self.synthetic_span_before_current());
                }
            }
            '}' => {
                if self.is_at_end() || self.is_block_terminator_token() {
                    return Some(self.synthetic_span_before_current());
                }
            }
            ')' => {
                if self.is_at_end()
                    || self.is_post_paren_boundary_token()
                    || self.is_statement_boundary_token()
                {
                    return Some(self.synthetic_span_before_current());
                }
            }
            _ => {}
        }

        None
    }

    fn recover_in_delimited_list(
        &mut self,
        terminator_symbols: &[char],
        separator_symbols: &[char],
    ) {
        while !self.is_at_end() {
            match &self.current().kind {
                TokenKind::Symbol(symbol)
                    if terminator_symbols.contains(symbol)
                        || separator_symbols.contains(symbol)
                        || matches!(symbol, '}' | ';') =>
                {
                    return;
                }
                TokenKind::Keyword(_) | TokenKind::EndOfFile => return,
                _ => self.advance(),
            }
        }
    }

    fn synthetic_span_before_current(&self) -> Span {
        if self.position == 0 {
            return Span::dummy();
        }

        let prev_span = self.tokens[self.position - 1].span;
        Span::new(
            prev_span.end,
            prev_span.end,
            prev_span.end_location,
            prev_span.end_location,
        )
    }

    fn is_statement_boundary_token(&self) -> bool {
        if self.is_at_end() {
            return true;
        }

        match &self.current().kind {
            TokenKind::Keyword(kw) => matches!(
                kw,
                Keyword::Let
                    | Keyword::Return
                    | Keyword::If
                    | Keyword::Unless
                    | Keyword::Match
                    | Keyword::While
                    | Keyword::Do
                    | Keyword::For
                    | Keyword::Loop
                    | Keyword::Switch
                    | Keyword::Break
                    | Keyword::Continue
                    | Keyword::Fn
                    | Keyword::Struct
                    | Keyword::Enum
                    | Keyword::Impl
                    | Keyword::Trait
                    | Keyword::Class
                    | Keyword::Module
                    | Keyword::Import
                    | Keyword::Pub
                    | Keyword::Case
                    | Keyword::Else
                    | Keyword::Elif
                    | Keyword::ElseIf
            ),
            TokenKind::Identifier(_) | TokenKind::Number(_) | TokenKind::StringLiteral(_) => true,
            TokenKind::Symbol('(') | TokenKind::Symbol('{') => true,
            _ => false,
        }
    }

    fn is_block_terminator_token(&self) -> bool {
        if self.is_at_end() {
            return true;
        }

        match &self.current().kind {
            TokenKind::Keyword(Keyword::Else)
            | TokenKind::Keyword(Keyword::Elif)
            | TokenKind::Keyword(Keyword::ElseIf)
            | TokenKind::Keyword(Keyword::Case)
            | TokenKind::Keyword(Keyword::Fn)
            | TokenKind::Keyword(Keyword::Struct)
            | TokenKind::Keyword(Keyword::Enum)
            | TokenKind::Keyword(Keyword::Trait)
            | TokenKind::Keyword(Keyword::Impl)
            | TokenKind::Keyword(Keyword::Class)
            | TokenKind::Keyword(Keyword::Module)
            | TokenKind::Keyword(Keyword::Import)
            | TokenKind::Keyword(Keyword::Return) => true,
            _ => false,
        }
    }

    fn is_post_paren_boundary_token(&self) -> bool {
        if self.is_at_end() {
            return true;
        }

        match &self.current().kind {
            TokenKind::Symbol('{') | TokenKind::Symbol(')') => true,
            TokenKind::Operator(crate::token::Operator::Arrow) => true,
            _ => false,
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
            TokenKind::CharLiteral(c) => format!("char literal '{}'", c),
            TokenKind::FStringLiteral(_) => "f-string literal".to_string(),
        }
    }

    fn keyword_hint(&self, keyword: Keyword) -> Option<String> {
        match keyword {
            Keyword::Module => Some("Start the file with `module <name>;`.".to_string()),
            Keyword::Import => {
                Some("Use `import path.to.module;` to bring other modules into scope.".to_string())
            }
            Keyword::Fn => Some("Function declarations start with `fn name(...)`.".to_string()),
            Keyword::Trait => {
                Some("Traits are declared with `trait TraitName { ... }`.".to_string())
            }
            Keyword::Impl => Some("Use `impl Type` to provide trait implementations.".to_string()),
            Keyword::Let => Some("Introduce bindings with `let name = expression;`.".to_string()),
            Keyword::Return => {
                Some("Use `return expression;` to exit a function early.".to_string())
            }
            Keyword::While | Keyword::For | Keyword::Loop => Some(
                "Loops require a control keyword such as `while`, `for`, or `loop`.".to_string(),
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Module;
    use crate::lexer::Lexer;
    use std::collections::HashSet;

    fn parse_with_features(source: &str, features: &[&str]) -> Result<Module, Vec<ParseError>> {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("lexer should not fail in parser tests");
        let mut feature_set = HashSet::new();
        for feature in features {
            feature_set.insert((*feature).to_string());
        }
        Parser::new(tokens, feature_set).parse()
    }

    #[test]
    fn loop_feature_flag_gates_parsing() {
        let source = r#"
            module demo;

            fn main() {
                loop {
                    break;
                }
            }
        "#;

        assert!(parse_with_features(source, &[]).is_err());
        assert!(parse_with_features(source, &["loop"]).is_ok());
    }

    #[test]
    fn unless_feature_flag_gates_parsing() {
        let source = r#"
            module demo;

            fn main() {
                let value = unless false { 1 };
            }
        "#;

        assert!(parse_with_features(source, &[]).is_err());
        assert!(parse_with_features(source, &["unless"]).is_ok());
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
            TypeAnnotationKind::Function { .. } => TypePattern::Simple(vec!["fn".to_string()]),
            TypeAnnotationKind::Generic { name, .. } => TypePattern::Simple(vec![name.clone()]),
            TypeAnnotationKind::DynTrait { trait_name } => {
                TypePattern::Simple(vec![format!("dyn {}", trait_name)])
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
