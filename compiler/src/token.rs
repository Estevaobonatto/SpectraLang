use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    Module,
    Import,
    Export,
    Pub,
    Use,
    Type,
    Struct,
    Enum,
    Union,
    Trait,
    Impl,
    Class,
    Extends,
    Implements,
    New,
    If,
    Else,
    Match,
    For,
    While,
    Break,
    Continue,
    Return,
    Defer,
    Using,
    Fn,
    Async,
    Await,
    Yield,
    Try,
    Catch,
    Throw,
    Let,
    Var,
    Macro,
    Comptime,
    Directive,
    On,
    Off,
    True,
    False,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Identifier,
    Integer,
    Float,
    String,
    Keyword(Keyword),
    Colon,
    Semicolon,
    Comma,
    Dot,
    Scope,
    Arrow,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Ampersand,
    Pipe,
    Unknown,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: String, span: Span) -> Self {
        Self { kind, lexeme, span }
    }
}

impl Keyword {
    pub fn from_identifier(ident: &str) -> Option<Self> {
        use Keyword::*;
        Some(match ident {
            "module" => Module,
            "import" => Import,
            "export" => Export,
            "pub" => Pub,
            "use" => Use,
            "type" => Type,
            "struct" => Struct,
            "enum" => Enum,
            "union" => Union,
            "trait" => Trait,
            "impl" => Impl,
            "class" => Class,
            "extends" => Extends,
            "implements" => Implements,
            "new" => New,
            "if" => If,
            "else" => Else,
            "match" => Match,
            "for" => For,
            "while" => While,
            "break" => Break,
            "continue" => Continue,
            "return" => Return,
            "defer" => Defer,
            "using" => Using,
            "fn" => Fn,
            "async" => Async,
            "await" => Await,
            "yield" => Yield,
            "try" => Try,
            "catch" => Catch,
            "throw" => Throw,
            "let" => Let,
            "var" => Var,
            "macro" => Macro,
            "comptime" => Comptime,
            "directive" => Directive,
            "on" => On,
            "off" => Off,
            "true" => True,
            "false" => False,
            _ => return None,
        })
    }
}

impl TokenKind {
    pub fn is_keyword(&self) -> bool {
        matches!(self, TokenKind::Keyword(_))
    }
}
