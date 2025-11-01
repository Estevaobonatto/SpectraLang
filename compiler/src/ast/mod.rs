use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    Unit,    // Tipo vazio (sem valor de retorno)
    Unknown, // Tipo desconhecido (para inferência)
    Array {
        element_type: Box<Type>,
        size: Option<usize>, // None = tamanho dinâmico
    },
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub span: Span,
    pub items: Vec<Item>,
}

impl Module {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Import(Import),
    Function(Function),
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub span: Span,
    pub visibility: Visibility,
    pub params: Vec<FunctionParam>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: String,
    pub span: Span,
    pub ty: Option<TypeAnnotation>,
}

#[derive(Debug, Clone)]
pub struct TypeAnnotation {
    pub segments: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub span: Span,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Let(LetStatement),
    Assignment(AssignmentStatement),
    Return(ReturnStatement),
    Expression(Expression),
    While(WhileLoop),
    DoWhile(DoWhileLoop),
    For(ForLoop),
    Loop(LoopStatement),
    Switch(SwitchStatement),
    Break,
    Continue,
}

#[derive(Debug, Clone)]
pub enum LValue {
    Identifier(String),
    IndexAccess {
        array: Box<Expression>,
        index: Box<Expression>,
    },
}

#[derive(Debug, Clone)]
pub struct AssignmentStatement {
    pub target: LValue,
    pub target_span: Span,
    pub value: Expression,
}

#[derive(Debug, Clone)]
pub struct WhileLoop {
    pub condition: Expression,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForLoop {
    pub iterator: String,
    pub iterable: Expression,
    pub body: Block,
    pub span: Span,
    pub is_in: bool, // true for 'in', false for 'of'
}

#[derive(Debug, Clone)]
pub struct LetStatement {
    pub name: String,
    pub span: Span,
    pub ty: Option<TypeAnnotation>,
    pub value: Option<Expression>,
}

#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub span: Span,
    pub value: Option<Expression>,
}

#[derive(Debug, Clone)]
pub struct Expression {
    pub span: Span,
    pub kind: ExpressionKind,
}

#[derive(Debug, Clone)]
pub enum ExpressionKind {
    // Literals
    Identifier(String),
    NumberLiteral(String),
    StringLiteral(String),
    BoolLiteral(bool),

    // Operations
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },

    // Function calls
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },

    // Control flow expressions
    If {
        condition: Box<Expression>,
        then_block: Block,
        elif_blocks: Vec<(Expression, Block)>,
        else_block: Option<Block>,
    },

    // Unless é como if, mas com condição negada
    Unless {
        condition: Box<Expression>,
        then_block: Block,
        else_block: Option<Block>,
    },

    // Grouping
    Grouping(Box<Expression>),

    // Arrays
    ArrayLiteral {
        elements: Vec<Expression>,
    },
    IndexAccess {
        array: Box<Expression>,
        index: Box<Expression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    // Arithmetic
    Add,      // +
    Subtract, // -
    Multiply, // *
    Divide,   // /
    Modulo,   // %

    // Comparison
    Equal,        // ==
    NotEqual,     // !=
    Less,         // <
    Greater,      // >
    LessEqual,    // <=
    GreaterEqual, // >=

    // Logical
    And, // &&
    Or,  // ||
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Negate, // -
    Not,    // !
}

// Loop infinito: loop { ... }
#[derive(Debug, Clone)]
pub struct LoopStatement {
    pub body: Block,
    pub span: Span,
}

// Do-while: do { ... } while condition;
#[derive(Debug, Clone)]
pub struct DoWhileLoop {
    pub body: Block,
    pub condition: Expression,
    pub span: Span,
}

// Switch: switch expr { case pattern => body, ... }
#[derive(Debug, Clone)]
pub struct SwitchStatement {
    pub value: Expression,
    pub cases: Vec<SwitchCase>,
    pub default: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SwitchCase {
    pub pattern: Expression, // Padrão a ser comparado
    pub body: Block,
    pub span: Span,
}
