use crate::span::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Keyword {
    // Module system
    Module,
    Import,
    Export,

    // Declarations
    Fn,
    Struct,
    Enum,
    Impl,
    Class,
    Trait,
    Let,

    // Visibility
    Pub,
    Internal,
    Mut,

    // Special types
    SelfType, // Self keyword for referring to implementing type

    // Control flow - conditionals
    Match,
    Switch,
    Case,
    Cond,
    If,
    Else,
    Elif,
    ElseIf,
    Unless,

    // Control flow - loops
    While,
    Do,
    For,
    Foreach,
    In,
    Of,
    Repeat,
    Until,
    Loop,

    // Control flow - jumps
    Return,
    Break,
    Continue,
    Yield,
    Goto,

    // Literals
    True,
    False,

    // Type/value declarations
    Type,
    Const,
    Static,

    // Type casting / dynamic dispatch
    As,
    Dyn,
}

impl Keyword {
    pub fn from_identifier(identifier: &str) -> Option<Self> {
        match identifier {
            // Module system
            "module" => Some(Self::Module),
            "import" => Some(Self::Import),
            "export" => Some(Self::Export),

            // Declarations
            "fn" => Some(Self::Fn),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "impl" => Some(Self::Impl),
            "class" => Some(Self::Class),
            "trait" => Some(Self::Trait),
            "let" => Some(Self::Let),

            // Visibility
            "pub" => Some(Self::Pub),
            "internal" => Some(Self::Internal),
            "mut" => Some(Self::Mut),

            // Special types
            "Self" => Some(Self::SelfType),

            // Control flow - conditionals
            "match" => Some(Self::Match),
            "switch" => Some(Self::Switch),
            "case" => Some(Self::Case),
            "cond" => Some(Self::Cond),
            "if" => Some(Self::If),
            "else" => Some(Self::Else),
            "elif" => Some(Self::Elif),
            "elseif" => Some(Self::ElseIf),
            "unless" => Some(Self::Unless),

            // Control flow - loops
            "while" => Some(Self::While),
            "do" => Some(Self::Do),
            "for" => Some(Self::For),
            "foreach" => Some(Self::Foreach),
            "in" => Some(Self::In),
            "of" => Some(Self::Of),
            "repeat" => Some(Self::Repeat),
            "until" => Some(Self::Until),
            "loop" => Some(Self::Loop),

            // Control flow - jumps
            "return" => Some(Self::Return),
            "break" => Some(Self::Break),
            "continue" => Some(Self::Continue),
            "yield" => Some(Self::Yield),
            "goto" => Some(Self::Goto),

            // Literals
            "true" => Some(Self::True),
            "false" => Some(Self::False),

            // Type/value declarations
            "type" => Some(Self::Type),
            "const" => Some(Self::Const),
            "static" => Some(Self::Static),

            // Type casting / dynamic dispatch
            "as" => Some(Self::As),
            "dyn" => Some(Self::Dyn),

            _ => None,
        }
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            // Module system
            Keyword::Module => "module",
            Keyword::Import => "import",
            Keyword::Export => "export",

            // Declarations
            Keyword::Fn => "fn",
            Keyword::Struct => "struct",
            Keyword::Enum => "enum",
            Keyword::Impl => "impl",
            Keyword::Class => "class",
            Keyword::Trait => "trait",
            Keyword::Let => "let",

            // Visibility
            Keyword::Pub => "pub",
            Keyword::Internal => "internal",
            Keyword::Mut => "mut",

            // Special types
            Keyword::SelfType => "Self",

            // Control flow - conditionals
            Keyword::Match => "match",
            Keyword::Switch => "switch",
            Keyword::Case => "case",
            Keyword::Cond => "cond",
            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::Elif => "elif",
            Keyword::ElseIf => "elseif",
            Keyword::Unless => "unless",

            // Control flow - loops
            Keyword::While => "while",
            Keyword::Do => "do",
            Keyword::For => "for",
            Keyword::Foreach => "foreach",
            Keyword::In => "in",
            Keyword::Of => "of",
            Keyword::Repeat => "repeat",
            Keyword::Until => "until",
            Keyword::Loop => "loop",

            // Control flow - jumps
            Keyword::Return => "return",
            Keyword::Break => "break",
            Keyword::Continue => "continue",
            Keyword::Yield => "yield",
            Keyword::Goto => "goto",

            // Literals
            Keyword::True => "true",
            Keyword::False => "false",

            // Type/value declarations
            Keyword::Type => "type",
            Keyword::Const => "const",
            Keyword::Static => "static",

            // Type casting / dynamic dispatch
            Keyword::As => "as",
            Keyword::Dyn => "dyn",
        };

        write!(f, "{}", text)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Number(String),
    Keyword(Keyword),
    Symbol(char),
    Operator(Operator),
    StringLiteral(String),
    /// Character literal: 'a', '\n', etc.
    CharLiteral(char),
    /// F-string raw template: f"Hello, {name}!"
    FStringLiteral(String),
    EndOfFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Operator {
    // Comparison
    EqualEqual,   // ==
    NotEqual,     // !=
    LessEqual,    // <=
    GreaterEqual, // >=

    // Logical
    And, // &&
    Or,  // ||

    // Arrows
    Arrow,    // -> (for function returns)
    FatArrow, // => (for match arms, etc.)

    // Range operators
    Range,          // ..
    RangeInclusive, // ..=
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

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Operator::EqualEqual => "==",
            Operator::NotEqual => "!=",
            Operator::LessEqual => "<=",
            Operator::GreaterEqual => ">=",
            Operator::And => "&&",
            Operator::Or => "||",
            Operator::Arrow => "->",
            Operator::FatArrow => "=>",
            Operator::Range => "..",
            Operator::RangeInclusive => "..=",
        };

        write!(f, "{}", text)
    }
}
