/// Tests for AST lowering to IR
use spectra_compiler::ast::{
    BinaryOperator, Block, Expression, ExpressionKind, Function, FunctionParam, Item,
    LetStatement, LoopStatement, Module, ReturnStatement, Statement, StatementKind, Visibility,
    WhileLoop,
};
use spectra_compiler::span::Span;
use spectra_midend::ir::InstructionKind;
use spectra_midend::ASTLowering;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn s() -> Span {
    Span::dummy()
}

fn int_lit(n: i64) -> Expression {
    Expression {
        span: s(),
        kind: ExpressionKind::NumberLiteral(n.to_string()),
    }
}

fn bool_lit(b: bool) -> Expression {
    Expression {
        span: s(),
        kind: ExpressionKind::BoolLiteral(b),
    }
}

fn ident(name: &str) -> Expression {
    Expression {
        span: s(),
        kind: ExpressionKind::Identifier(name.to_string()),
    }
}

fn bin(left: Expression, op: BinaryOperator, right: Expression) -> Expression {
    Expression {
        span: s(),
        kind: ExpressionKind::Binary {
            left: Box::new(left),
            operator: op,
            right: Box::new(right),
        },
    }
}

fn let_stmt(name: &str, value: Expression) -> Statement {
    Statement {
        span: s(),
        kind: StatementKind::Let(LetStatement {
            name: name.to_string(),
            span: s(),
            ty: None,
            value: Some(value),
        }),
    }
}

fn return_stmt(value: Expression) -> Statement {
    Statement {
        span: s(),
        kind: StatementKind::Return(ReturnStatement {
            span: s(),
            value: Some(value),
        }),
    }
}

fn make_function(name: &str, stmts: Vec<Statement>) -> Item {
    Item::Function(Function {
        name: name.to_string(),
        span: s(),
        visibility: Visibility::Public,
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: Block {
            span: s(),
            statements: stmts,
        },
    })
}

fn make_function_with_params(
    name: &str,
    params: Vec<(&str, spectra_compiler::ast::TypeAnnotation)>,
    stmts: Vec<Statement>,
    return_type: Option<spectra_compiler::ast::TypeAnnotation>,
) -> Item {
    Item::Function(Function {
        name: name.to_string(),
        span: s(),
        visibility: Visibility::Public,
        type_params: vec![],
        params: params
            .into_iter()
            .map(|(n, ty)| FunctionParam {
                name: n.to_string(),
                span: s(),
                ty: Some(ty),
            })
            .collect(),
        return_type,
        body: Block {
            span: s(),
            statements: stmts,
        },
    })
}

fn int_type() -> spectra_compiler::ast::TypeAnnotation {
    use spectra_compiler::ast::{TypeAnnotation, TypeAnnotationKind};
    TypeAnnotation {
        kind: TypeAnnotationKind::Simple {
            segments: vec!["int".to_string()],
        },
        span: s(),
    }
}

fn make_module(name: &str, items: Vec<Item>) -> Module {
    Module {
        name: name.to_string(),
        span: s(),
        items,
        std_import_aliases: Vec::new(),
        imported_function_return_types: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_lower_simple_arithmetic() {
    // let x = 5 + 3;
    let module = make_module(
        "test",
        vec![make_function(
            "main",
            vec![let_stmt(
                "x",
                bin(int_lit(5), BinaryOperator::Add, int_lit(3)),
            )],
        )],
    );

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    assert_eq!(ir_module.name, "test");
    assert_eq!(ir_module.functions.len(), 1);

    let func = &ir_module.functions[0];
    assert_eq!(func.name, "main");
    assert!(!func.blocks.is_empty());

    let entry_block = &func.blocks[0];
    // Should have at least: ConstInt(5), ConstInt(3), Add
    assert!(
        entry_block.instructions.len() >= 3,
        "Expected >= 3 instructions, got {}",
        entry_block.instructions.len()
    );
}

#[test]
fn test_lower_multiple_operations() {
    // let a = 10 - 4;
    // let b = a * 2;
    let module = make_module(
        "test",
        vec![make_function(
            "main",
            vec![
                let_stmt("a", bin(int_lit(10), BinaryOperator::Subtract, int_lit(4))),
                let_stmt(
                    "b",
                    bin(ident("a"), BinaryOperator::Multiply, int_lit(2)),
                ),
            ],
        )],
    );

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    let func = &ir_module.functions[0];
    assert!(!func.blocks.is_empty());

    let entry_block = &func.blocks[0];
    let has_sub = entry_block
        .instructions
        .iter()
        .any(|i| matches!(i.kind, InstructionKind::Sub { .. }));
    let has_mul = entry_block
        .instructions
        .iter()
        .any(|i| matches!(i.kind, InstructionKind::Mul { .. }));
    assert!(has_sub, "Should have Sub instruction");
    assert!(has_mul, "Should have Mul instruction");
}

#[test]
fn test_lower_while_loop() {
    // while true { }
    let module = make_module(
        "test",
        vec![make_function(
            "main",
            vec![Statement {
                span: s(),
                kind: StatementKind::While(WhileLoop {
                    condition: bool_lit(true),
                    body: Block {
                        span: s(),
                        statements: vec![Statement {
                            span: s(),
                            kind: StatementKind::Break,
                        }],
                    },
                    span: s(),
                }),
            }],
        )],
    );

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    let func = &ir_module.functions[0];
    // while loop generates at least: header block + body block + exit block
    assert!(
        func.blocks.len() >= 2,
        "While loop should generate multiple blocks, got {}",
        func.blocks.len()
    );
}

#[test]
fn test_lower_loop_infinite() {
    // loop { break; }
    let module = make_module(
        "test",
        vec![make_function(
            "main",
            vec![Statement {
                span: s(),
                kind: StatementKind::Loop(LoopStatement {
                    body: Block {
                        span: s(),
                        statements: vec![Statement {
                            span: s(),
                            kind: StatementKind::Break,
                        }],
                    },
                    span: s(),
                }),
            }],
        )],
    );

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    let func = &ir_module.functions[0];
    // loop produces at least a header block and a body block
    assert!(!func.blocks.is_empty());
}

#[test]
fn test_lower_function_call() {
    // fn add(a: int, b: int) -> int { return a + b; }
    // fn main() { let r = add(5, 3); }
    let add_fn = make_function_with_params(
        "add",
        vec![("a", int_type()), ("b", int_type())],
        vec![return_stmt(bin(
            ident("a"),
            BinaryOperator::Add,
            ident("b"),
        ))],
        Some(int_type()),
    );

    let call_expr = Expression {
        span: s(),
        kind: ExpressionKind::Call {
            callee: Box::new(ident("add")),
            arguments: vec![int_lit(5), int_lit(3)],
        },
    };
    let main_fn = make_function("main", vec![let_stmt("r", call_expr)]);

    let module = make_module("test", vec![add_fn, main_fn]);

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    assert_eq!(ir_module.functions.len(), 2);

    // Find main function and verify it has a Call instruction
    let main_ir = ir_module
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main function should exist");

    let has_call = main_ir.blocks.iter().any(|b| {
        b.instructions
            .iter()
            .any(|i| matches!(i.kind, InstructionKind::Call { .. }))
    });
    assert!(has_call, "Should have Call instruction");
}

#[test]
fn test_lower_boolean_literals() {
    // let t = true;
    // let f = false;
    let module = make_module(
        "test",
        vec![make_function(
            "main",
            vec![
                let_stmt("t", bool_lit(true)),
                let_stmt("f", bool_lit(false)),
            ],
        )],
    );

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    let func = &ir_module.functions[0];
    let entry = &func.blocks[0];

    let has_bool_true = entry
        .instructions
        .iter()
        .any(|i| matches!(i.kind, InstructionKind::ConstBool { value: true, .. }));
    let has_bool_false = entry
        .instructions
        .iter()
        .any(|i| matches!(i.kind, InstructionKind::ConstBool { value: false, .. }));

    assert!(has_bool_true, "Should have ConstBool(true)");
    assert!(has_bool_false, "Should have ConstBool(false)");
}

#[test]
fn test_lower_comparison() {
    // let x = 5 > 3;
    let module = make_module(
        "test",
        vec![make_function(
            "main",
            vec![let_stmt(
                "x",
                bin(int_lit(5), BinaryOperator::Greater, int_lit(3)),
            )],
        )],
    );

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    let func = &ir_module.functions[0];
    let entry = &func.blocks[0];

    let has_gt = entry
        .instructions
        .iter()
        .any(|i| matches!(i.kind, InstructionKind::Gt { .. }));
    assert!(has_gt, "Should have Gt (greater-than) instruction");
}

#[test]
fn test_lower_empty_function() {
    // fn empty() { }
    let module = make_module("test", vec![make_function("empty", vec![])]);

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    assert_eq!(ir_module.functions.len(), 1);
    let func = &ir_module.functions[0];
    assert_eq!(func.name, "empty");
    assert!(!func.blocks.is_empty());
}

#[test]
fn test_lower_multiple_functions() {
    let module = make_module(
        "test",
        vec![
            make_function("foo", vec![]),
            make_function("bar", vec![]),
            make_function("baz", vec![]),
        ],
    );

    let mut lowering = ASTLowering::new();
    let ir_module = lowering.lower_module(&module).expect("lowering should succeed");

    assert_eq!(ir_module.functions.len(), 3);
    assert!(ir_module.functions.iter().any(|f| f.name == "foo"));
    assert!(ir_module.functions.iter().any(|f| f.name == "bar"));
    assert!(ir_module.functions.iter().any(|f| f.name == "baz"));
}
