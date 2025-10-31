use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Keyword {
    Module,
    Import,
    Export,
    Fn,
    Class,
    Trait,
    Let,
    Pub,
    Match,
    If,
    Else,
    Elif,
    While,
    For,
    Loop,
    Return,
}

impl Keyword {
    pub fn from_identifier(identifier: &str) -> Option<Self> {
        match identifier {
            "module" => Some(Self::Module),
            "import" => Some(Self::Import),
            "export" => Some(Self::Export),
            "fn" => Some(Self::Fn),
            "class" => Some(Self::Class),
            "trait" => Some(Self::Trait),
            "let" => Some(Self::Let),
            "pub" => Some(Self::Pub),
            "match" => Some(Self::Match),
            "if" => Some(Self::If),
            "else" => Some(Self::Else),
            "elif" => Some(Self::Elif),
            "while" => Some(Self::While),
            "for" => Some(Self::For),
            "loop" => Some(Self::Loop),
            "return" => Some(Self::Return),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Number(String),
    Keyword(Keyword),
    Symbol(char),
    StringLiteral(String),
    EndOfFile,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
