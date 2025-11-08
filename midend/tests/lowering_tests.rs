/// Tests for optimization passes
use spectra_midend::ir::*;
use spectra_midend::passes::constant_folding::ConstantFoldingPass;
use spectra_midend::passes::dead_code_elimination::DeadCodeEliminationPass;
use spectra_midend::passes::Pass;

fn dummy_span() -> Span {
    Span {
        start: 0,
        end: 0,
        start_location: Location { line: 1, column: 1 },
        end_location: Location { line: 1, column: 1 },
    }
}

#[test]
fn test_lower_simple_arithmetic() {
    // let x = 5 + 3;
    let module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "main".to_string(),
            parameters: vec![],
            return_type: TypeAnnotation::Void,
            body: vec![Statement::Let {
                name: "x".to_string(),
                type_annotation: None,
                value: Some(Expression::Binary {
                    left: Box::new(Expression::IntLiteral {
                        value: 5,
                        span: dummy_span(),
                    }),
                    operator: BinaryOperator::Add,
                    right: Box::new(Expression::IntLiteral {
                        value: 3,
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            span: dummy_span(),
            is_public: true,
        }],
        span: dummy_span(),
    };

    let mut lowering = ASTLowering::new();
    let ir_module = lowering
        .lower_module(&module)
        .expect("Lowering should succeed");

    assert_eq!(ir_module.name, "test");
    assert_eq!(ir_module.functions.len(), 1);

    let func = &ir_module.functions[0];
    assert_eq!(func.name, "main");
    assert_eq!(func.blocks.len(), 1);

    let entry_block = &func.blocks[0];
    // Should have: ConstInt(5), ConstInt(3), Add
    assert!(entry_block.instructions.len() >= 3);
}

#[test]
fn test_lower_if_expression() {
    // if x > 0 { 10 } else { 20 }
    let module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "main".to_string(),
            parameters: vec![],
            return_type: TypeAnnotation::Void,
            body: vec![Statement::Let {
                name: "result".to_string(),
                type_annotation: None,
                value: Some(Expression::If {
                    condition: Box::new(Expression::Binary {
                        left: Box::new(Expression::Variable {
                            name: "x".to_string(),
                            span: dummy_span(),
                        }),
                        operator: BinaryOperator::Greater,
                        right: Box::new(Expression::IntLiteral {
                            value: 0,
                            span: dummy_span(),
                        }),
                        span: dummy_span(),
                    }),
                    then_branch: Box::new(Expression::IntLiteral {
                        value: 10,
                        span: dummy_span(),
                    }),
                    else_branch: Some(Box::new(Expression::IntLiteral {
                        value: 20,
                        span: dummy_span(),
                    })),
                    span: dummy_span(),
                }),
                span: dummy_span(),
            }],
            span: dummy_span(),
            is_public: true,
        }],
        span: dummy_span(),
    };

    let mut lowering = ASTLowering::new();
    let ir_module = lowering
        .lower_module(&module)
        .expect("Lowering should succeed");

    let func = &ir_module.functions[0];
    // Should have multiple blocks: entry, then, else, merge
    assert!(func.blocks.len() >= 3);
}

#[test]
fn test_lower_while_loop() {
    // while x < 10 { x = x + 1; }
    let module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "main".to_string(),
            parameters: vec![],
            return_type: TypeAnnotation::Void,
            body: vec![Statement::While {
                condition: Expression::Binary {
                    left: Box::new(Expression::Variable {
                        name: "x".to_string(),
                        span: dummy_span(),
                    }),
                    operator: BinaryOperator::Less,
                    right: Box::new(Expression::IntLiteral {
                        value: 10,
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                },
                body: vec![Statement::Assignment {
                    target: "x".to_string(),
                    value: Expression::Binary {
                        left: Box::new(Expression::Variable {
                            name: "x".to_string(),
                            span: dummy_span(),
                        }),
                        operator: BinaryOperator::Add,
                        right: Box::new(Expression::IntLiteral {
                            value: 1,
                            span: dummy_span(),
                        }),
                        span: dummy_span(),
                    },
                    span: dummy_span(),
                }],
                span: dummy_span(),
            }],
            span: dummy_span(),
            is_public: true,
        }],
        span: dummy_span(),
    };

    let mut lowering = ASTLowering::new();
    let ir_module = lowering
        .lower_module(&module)
        .expect("Lowering should succeed");

    let func = &ir_module.functions[0];
    // Should have blocks: entry, header, body, exit
    assert!(func.blocks.len() >= 3);
}

#[test]
fn test_lower_function_call() {
    // add(5, 3)
    let module = Module {
        name: "test".to_string(),
        functions: vec![
            Function {
                name: "add".to_string(),
                parameters: vec![
                    Parameter {
                        name: "a".to_string(),
                        type_annotation: TypeAnnotation::Int,
                        span: dummy_span(),
                    },
                    Parameter {
                        name: "b".to_string(),
                        type_annotation: TypeAnnotation::Int,
                        span: dummy_span(),
                    },
                ],
                return_type: TypeAnnotation::Int,
                body: vec![Statement::Return {
                    value: Some(Expression::Binary {
                        left: Box::new(Expression::Variable {
                            name: "a".to_string(),
                            span: dummy_span(),
                        }),
                        operator: BinaryOperator::Add,
                        right: Box::new(Expression::Variable {
                            name: "b".to_string(),
                            span: dummy_span(),
                        }),
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }],
                span: dummy_span(),
                is_public: false,
            },
            Function {
                name: "main".to_string(),
                parameters: vec![],
                return_type: TypeAnnotation::Void,
                body: vec![Statement::Let {
                    name: "result".to_string(),
                    type_annotation: None,
                    value: Some(Expression::Call {
                        function: "add".to_string(),
                        arguments: vec![
                            Expression::IntLiteral {
                                value: 5,
                                span: dummy_span(),
                            },
                            Expression::IntLiteral {
                                value: 3,
                                span: dummy_span(),
                            },
                        ],
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                }],
                span: dummy_span(),
                is_public: true,
            },
        ],
        span: dummy_span(),
    };

    let mut lowering = ASTLowering::new();
    let ir_module = lowering
        .lower_module(&module)
        .expect("Lowering should succeed");

    assert_eq!(ir_module.functions.len(), 2);

    let main_func = &ir_module.functions[1];
    let entry_block = &main_func.blocks[0];

    // Should have Call instruction
    let has_call = entry_block
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, InstructionKind::Call { .. }));
    assert!(has_call, "Should have Call instruction");
}

#[test]
fn test_lower_break_continue() {
    // loop { if x > 10 { break; } x = x + 1; continue; }
    let module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "main".to_string(),
            parameters: vec![],
            return_type: TypeAnnotation::Void,
            body: vec![Statement::Loop {
                body: vec![
                    Statement::If {
                        condition: Expression::Binary {
                            left: Box::new(Expression::Variable {
                                name: "x".to_string(),
                                span: dummy_span(),
                            }),
                            operator: BinaryOperator::Greater,
                            right: Box::new(Expression::IntLiteral {
                                value: 10,
                                span: dummy_span(),
                            }),
                            span: dummy_span(),
                        },
                        then_branch: vec![Statement::Break { span: dummy_span() }],
                        else_branch: None,
                        span: dummy_span(),
                    },
                    Statement::Continue { span: dummy_span() },
                ],
                span: dummy_span(),
            }],
            span: dummy_span(),
            is_public: true,
        }],
        span: dummy_span(),
    };

    let mut lowering = ASTLowering::new();
    let result = lowering.lower_module(&module);

    // Should succeed - break/continue are valid inside loop
    assert!(result.is_ok(), "Break/Continue should work inside loop");
}

#[test]
fn test_lower_constants() {
    // Test all constant types
    let module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "main".to_string(),
            parameters: vec![],
            return_type: TypeAnnotation::Void,
            body: vec![
                Statement::Let {
                    name: "i".to_string(),
                    type_annotation: None,
                    value: Some(Expression::IntLiteral {
                        value: 42,
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                },
                Statement::Let {
                    name: "f".to_string(),
                    type_annotation: None,
                    value: Some(Expression::FloatLiteral {
                        value: 3.14,
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                },
                Statement::Let {
                    name: "b".to_string(),
                    type_annotation: None,
                    value: Some(Expression::BoolLiteral {
                        value: true,
                        span: dummy_span(),
                    }),
                    span: dummy_span(),
                },
            ],
            span: dummy_span(),
            is_public: true,
        }],
        span: dummy_span(),
    };

    let mut lowering = ASTLowering::new();
    let ir_module = lowering
        .lower_module(&module)
        .expect("Lowering should succeed");

    let func = &ir_module.functions[0];
    let entry_block = &func.blocks[0];

    // Should have ConstInt, ConstFloat, ConstBool
    let has_const_int = entry_block
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, InstructionKind::ConstInt { .. }));
    let has_const_float = entry_block
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, InstructionKind::ConstFloat { .. }));
    let has_const_bool = entry_block
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, InstructionKind::ConstBool { .. }));

    assert!(has_const_int, "Should have ConstInt instruction");
    assert!(has_const_float, "Should have ConstFloat instruction");
    assert!(has_const_bool, "Should have ConstBool instruction");
}
