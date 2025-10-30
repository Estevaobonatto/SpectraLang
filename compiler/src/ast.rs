use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal {
        value: Literal,
        span: Span,
    },
    Identifier {
        name: String,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
        span: Span,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        operator: BinaryOperator,
        right: Box<Expr>,
        span: Span,
    },
    Grouping {
        expression: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
}

impl Visibility {
    pub fn is_public(self) -> bool {
        matches!(self, Visibility::Public)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        mutable: bool,
        name: String,
        value: Expr,
        span: Span,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: Option<ModulePath>,
    pub items: Vec<Item>,
}

impl Module {
    pub fn new(name: Option<ModulePath>, items: Vec<Item>) -> Self {
        Self { name, items }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModulePath {
    pub segments: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Import(Import),
    Export(Export),
    Stmt(Stmt),
    Function(Function),
    Constant(Constant),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub path: ModulePath,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    pub module_path: ModulePath,
    pub symbol: String,
    pub symbol_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
    pub name: String,
    pub value: Expr,
    pub mutable: bool,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeName>,
    pub body: Block,
    pub visibility: Visibility,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: TypeName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeName {
    pub segments: Vec<String>,
    pub span: Span,
}
