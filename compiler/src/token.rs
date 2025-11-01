use crate::span::Span;

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
    Class,
    Trait,
    Let,

    // Visibility
    Pub,
    Mut,

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
            "class" => Some(Self::Class),
            "trait" => Some(Self::Trait),
            "let" => Some(Self::Let),

            // Visibility
            "pub" => Some(Self::Pub),
            "mut" => Some(Self::Mut),

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
    Operator(Operator),
    StringLiteral(String),
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

    // Arrow (for function returns)
    Arrow, // ->
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
