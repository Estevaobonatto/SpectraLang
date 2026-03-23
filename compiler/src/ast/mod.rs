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
    Tuple {
        elements: Vec<Type>,
    },
    Struct {
        name: String,
    },
    Enum {
        name: String,
    },
    /// Generic type parameter (e.g., T in fn foo<T>(x: T))
    TypeParameter {
        name: String,
    },
    /// Self type - refers to the implementing type in trait methods or impl blocks
    SelfType,
    /// Closure/function type: fn(int, string) -> bool
    Fn {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub span: Span,
    pub items: Vec<Item>,
    /// Maps an unqualified function/type name imported from a stdlib module to the
    /// full stdlib path segments.  Populated by the semantic analyser when it
    /// processes `import std.io;` style declarations so that the midend can emit
    /// the correct host-function call even for bare identifiers.
    /// e.g. "print" -> ["std", "io", "print"]
    pub std_import_aliases: Vec<(String, Vec<String>)>,
    /// Return types for user-defined functions imported from other modules.
    /// Populated by the semantic analyser during import analysis so the midend
    /// can pre-register the return types and generate correct cross-module calls.
    /// e.g. ("square", Type::Int) when `import mathutils;` brings in `square`.
    pub imported_function_return_types: Vec<(String, Type)>,
}

impl Module {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
            items: Vec::new(),
            std_import_aliases: Vec::new(),
            imported_function_return_types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Import(Import),
    Function(Function),
    Struct(Struct),
    Enum(Enum),
    Impl(ImplBlock),
    Trait(TraitDeclaration),
    TraitImpl(TraitImpl),
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<String>,
    pub span: Span,
    /// Optional alias: `import path as alias`
    pub alias: Option<String>,
    /// Named imports: `import { name1, name2 } from path`
    pub names: Option<Vec<String>>,
    /// True when written as `pub import ...` — this is a re-export: the
    /// imported symbols are also exposed to whoever imports *this* module.
    pub is_reexport: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Accessible from any module that imports this one.
    Public,
    /// Accessible only within the same package (same `spectra.toml` `name`).
    Internal,
    /// Accessible only within this module (default).
    Private,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub span: Span,
    pub visibility: Visibility,
    pub type_params: Vec<TypeParameter>, // NEW: Generic type parameters
    pub params: Vec<FunctionParam>,
    pub return_type: Option<TypeAnnotation>,
    pub body: Block,
}

/// Generic type parameter: T, T: Trait, T: Trait1 + Trait2
#[derive(Debug, Clone)]
pub struct TypeParameter {
    pub name: String,
    pub bounds: Vec<String>, // Trait bounds (e.g., ["Printable", "Debug"])
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: String,
    pub span: Span,
    pub ty: Option<TypeAnnotation>,
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub span: Span,
    pub visibility: Visibility,
    pub fields: Vec<StructField>,
    pub type_params: Vec<TypeParameter>, // Generic type parameters (e.g., <T, U>)
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub span: Span,
    pub ty: TypeAnnotation,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub span: Span,
    pub visibility: Visibility,
    pub variants: Vec<EnumVariant>,
    pub type_params: Vec<TypeParameter>, // Generic type parameters (e.g., <T>)
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub span: Span,
    pub data: Option<Vec<TypeAnnotation>>,   // None for unit variants, Some for tuple variants
    /// Struct-style variant fields: `Variant { field: Type }`. When set, `data` is None.
    pub struct_data: Option<Vec<(String, TypeAnnotation)>>,
}

#[derive(Debug, Clone)]
pub struct TypeAnnotation {
    pub kind: TypeAnnotationKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeAnnotationKind {
    /// Tipo simples: int, string, etc.
    Simple { segments: Vec<String> },
    /// Tipo tuple: (int, string, bool)
    Tuple { elements: Vec<TypeAnnotation> },
    /// Function type: fn(T1, T2) -> ReturnType
    Function {
        params: Vec<TypeAnnotation>,
        return_type: Box<TypeAnnotation>,
    },
    /// Generic type application: Option<int>, Result<int, string>
    Generic {
        name: String,
        type_args: Vec<TypeAnnotation>,
    },
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
    /// `if let Pattern = expr { ... } [else { ... }]`
    IfLet(IfLetStatement),
    /// `while let Pattern = expr { ... }`
    WhileLet(WhileLetStatement),
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
    // Note: both `for x in expr` and `for x of expr` have identical semantics.
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
    /// Character literal: 'a', '\n', etc.
    CharLiteral(char),
    /// F-string with interpolated parts: f"Hello, {name}!"
    FString(Vec<FStringPart>),
    /// Block expression: { stmt; stmt; expr }
    Block(Block),

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
    /// Error propagation operator: expr?
    Try(Box<Expression>),
    /// Range: start..end (exclusive) or start..=end (inclusive)
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        inclusive: bool,
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

    // Tuples
    TupleLiteral {
        elements: Vec<Expression>,
    },
    TupleAccess {
        tuple: Box<Expression>,
        index: usize, // 0, 1, 2, etc.
    },

    // Structs
    StructLiteral {
        name: String,
        type_args: Vec<TypeAnnotation>, // Type arguments for generic instantiation: Point<int>
        fields: Vec<(String, Expression)>, // (field_name, value)
    },
    FieldAccess {
        object: Box<Expression>,
        field: String,
    },

    // Enums
    EnumVariant {
        enum_name: String,
        type_args: Vec<TypeAnnotation>, // Generic type arguments: Option<int>
        variant_name: String,
        data: Option<Vec<Expression>>, // None for unit, Some for tuple variants
        struct_data: Option<Vec<(String, Expression)>>, // Named fields for struct variants
    },

    // Pattern Matching
    Match {
        scrutinee: Box<Expression>,
        arms: Vec<MatchArm>,
    },

    // Method calls (dot notation)
    MethodCall {
        object: Box<Expression>,
        method_name: String,
        arguments: Vec<Expression>,
        type_name: Option<String>, // Preenchido pelo semantic analyzer
    },

    /// Closure/lambda: |params| expr  or  |params| { body }
    Lambda {
        params: Vec<LambdaParam>,
        body: Box<Expression>,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expression,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    // Wildcard pattern: _
    Wildcard,

    // Literal patterns: 42, true, "hello"
    Literal(Expression),

    // Identifier pattern: x (binds value)
    Identifier(String),

    // Enum variant patterns: Option::Some(x), Color::Red
    EnumVariant {
        enum_name: String,
        type_args: Vec<TypeAnnotation>, // Generic type arguments: Option<int>
        variant_name: String,
        data: Option<Vec<Pattern>>, // None for unit, Some(patterns) for tuple
        struct_data: Option<Vec<(String, Pattern)>>, // Named fields for struct variants
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

/// A single segment of an f-string template.
#[derive(Debug, Clone)]
pub enum FStringPart {
    /// Literal text segment
    Literal(String),
    /// Interpolated expression: `{expr}`
    Interpolated(Box<Expression>),
}

/// Parameter of a lambda/closure expression.
#[derive(Debug, Clone)]
pub struct LambdaParam {
    pub name: String,
    pub ty: Option<TypeAnnotation>,
    pub span: Span,
}

/// `if let Pattern = expr { ... } [else { ... }]`
#[derive(Debug, Clone)]
pub struct IfLetStatement {
    pub pattern: Pattern,
    pub value: Expression,
    pub then_block: Block,
    pub else_block: Option<Block>,
    pub span: Span,
}

/// `while let Pattern = expr { ... }`
#[derive(Debug, Clone)]
pub struct WhileLetStatement {
    pub pattern: Pattern,
    pub value: Expression,
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

// ============================================================================
// Impl Blocks e Methods
// ============================================================================

/// Bloco de implementação para adicionar métodos a um tipo
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub type_name: String,          // Nome do tipo (struct ou enum)
    pub trait_name: Option<String>, // Nome do trait (se for impl Trait for Type)
    pub methods: Vec<Method>,       // Métodos implementados
    pub span: Span,
}

/// Método associado a um tipo
#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    pub params: Vec<Parameter>, // Parâmetros (incluindo self, se presente)
    pub return_type: Option<TypeAnnotation>,
    pub body: Block,
    pub span: Span,
}

/// Parâmetro de método (pode ser self, &self, &mut self, ou parâmetro normal)
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub type_annotation: Option<TypeAnnotation>,
    pub is_self: bool,      // true se for self/&self/&mut self
    pub is_reference: bool, // true se for &self ou &mut self
    pub is_mutable: bool,   // true se for &mut self
    pub span: Span,
}

// ============================================================================
// Traits
// ============================================================================

/// Declaração de trait (interface)
#[derive(Debug, Clone)]
pub struct TraitDeclaration {
    pub name: String,
    pub parent_traits: Vec<String>, // NEW: Trait inheritance (e.g., trait Debug: Printable)
    pub methods: Vec<TraitMethod>,  // Assinaturas de métodos (sem corpo)
    pub span: Span,
}

/// Método em um trait (pode ter ou não implementação default)
#[derive(Debug, Clone)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Parameter>, // Parâmetros (incluindo self)
    pub return_type: Option<TypeAnnotation>,
    pub body: Option<Block>, // NEW: None = apenas assinatura, Some = default implementation
    pub span: Span,
}

/// Implementação de trait para um tipo: impl TraitName for TypeName
#[derive(Debug, Clone)]
pub struct TraitImpl {
    pub trait_name: String,   // Nome do trait
    pub type_name: String,    // Nome do tipo que implementa o trait
    pub methods: Vec<Method>, // Métodos implementados (com corpo)
    pub span: Span,
}
