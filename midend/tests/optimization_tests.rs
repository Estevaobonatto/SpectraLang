/// Tests for optimization passes on IR
use spectra_midend::ir::*;
use spectra_midend::passes::constant_folding;
use spectra_midend::passes::dead_code_elimination;

#[test]
fn test_constant_folding_add() {
    // Create a simple module with: x = 5 + 3
    let mut module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "test_func".to_string(),
            params: vec![],
            return_type: Type::Void,
            blocks: vec![BasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 0 },
                            value: 5,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 1 },
                            value: 3,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::Add {
                            result: Value { id: 2 },
                            lhs: Value { id: 0 },
                            rhs: Value { id: 1 },
                        },
                    },
                ],
                terminator: Some(Terminator::Return { value: None }),
            }],
        }],
    };

    // Apply constant folding
    let modified = constant_folding::run(&mut module);
    
    assert!(modified, "Constant folding should modify the module");
    
    // Check that Add was replaced with ConstInt(8)
    let func = &module.functions[0];
    let block = &func.blocks[0];
    
    let has_const_8 = block.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            InstructionKind::ConstInt { value: 8, .. }
        )
    });
    
    assert!(has_const_8, "Should have ConstInt(8) after folding");
}

#[test]
fn test_constant_folding_mul() {
    // Create: x = 10 * 2
    let mut module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "test_func".to_string(),
            params: vec![],
            return_type: Type::Void,
            blocks: vec![BasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 0 },
                            value: 10,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 1 },
                            value: 2,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::Mul {
                            result: Value { id: 2 },
                            lhs: Value { id: 0 },
                            rhs: Value { id: 1 },
                        },
                    },
                ],
                terminator: Some(Terminator::Return { value: None }),
            }],
        }],
    };

    let modified = constant_folding::run(&mut module);
    assert!(modified, "Constant folding should modify the module");
    
    let func = &module.functions[0];
    let block = &func.blocks[0];
    
    let has_const_20 = block.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            InstructionKind::ConstInt { value: 20, .. }
        )
    });
    
    assert!(has_const_20, "Should have ConstInt(20) after folding 10*2");
}

#[test]
fn test_dead_code_elimination_basic() {
    // Create module with unused computation
    let mut module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "test_func".to_string(),
            params: vec![],
            return_type: Type::Void,
            blocks: vec![BasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 0 },
                            value: 10,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 1 },
                            value: 20,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::Add {
                            result: Value { id: 2 },
                            lhs: Value { id: 0 },
                            rhs: Value { id: 1 },
                        },
                    },
                    // Result is never used - all code is dead
                ],
                terminator: Some(Terminator::Return { value: None }),
            }],
        }],
    };

    let initial_count = module.functions[0].blocks[0].instructions.len();
    
    let modified = dead_code_elimination::run(&mut module);
    assert!(modified, "DCE should modify the module");
    
    let final_count = module.functions[0].blocks[0].instructions.len();
    assert!(final_count < initial_count, "DCE should remove unused instructions");
}

#[test]
fn test_dead_code_elimination_preserves_used() {
    // Create module with used value
    let mut module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "test_func".to_string(),
            params: vec![],
            return_type: Type::Int,
            blocks: vec![BasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 0 },
                            value: 42,
                        },
                    },
                ],
                terminator: Some(Terminator::Return {
                    value: Some(Value { id: 0 }),
                }),
            }],
        }],
    };

    let initial_count = module.functions[0].blocks[0].instructions.len();
    
    let modified = dead_code_elimination::run(&mut module);
    
    let final_count = module.functions[0].blocks[0].instructions.len();
    assert_eq!(
        final_count, initial_count,
        "DCE should preserve used values"
    );
}

#[test]
fn test_combined_optimizations() {
    // Test constant folding followed by DCE
    let mut module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "test_func".to_string(),
            params: vec![],
            return_type: Type::Void,
            blocks: vec![BasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 0 },
                            value: 5,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::ConstInt {
                            result: Value { id: 1 },
                            value: 3,
                        },
                    },
                    Instruction {
                        kind: InstructionKind::Add {
                            result: Value { id: 2 },
                            lhs: Value { id: 0 },
                            rhs: Value { id: 1 },
                        },
                    },
                ],
                terminator: Some(Terminator::Return { value: None }),
            }],
        }],
    };

    // First pass: constant folding
    let cf_modified = constant_folding::run(&mut module);
    assert!(cf_modified, "Constant folding should apply");
    
    // Second pass: dead code elimination
    let dce_modified = dead_code_elimination::run(&mut module);
    assert!(dce_modified, "DCE should remove folded constants");
    
    // Result should have minimal instructions
    let final_count = module.functions[0].blocks[0].instructions.len();
    assert!(
        final_count == 0,
        "Combined optimizations should eliminate all dead code"
    );
}

#[test]
fn test_no_optimization_when_not_applicable() {
    // Test that passes don't modify code unnecessarily
    let mut module = Module {
        name: "test".to_string(),
        functions: vec![Function {
            name: "test_func".to_string(),
            params: vec![],
            return_type: Type::Int,
            blocks: vec![BasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    // Non-constant operation
                    Instruction {
                        kind: InstructionKind::Add {
                            result: Value { id: 2 },
                            lhs: Value { id: 0 }, // Function parameter
                            rhs: Value { id: 1 }, // Function parameter
                        },
                    },
                ],
                terminator: Some(Terminator::Return {
                    value: Some(Value { id: 2 }),
                }),
            }],
        }],
    };

    let cf_modified = constant_folding::run(&mut module);
    assert!(!cf_modified, "Constant folding should not apply");
    
    let dce_modified = dead_code_elimination::run(&mut module);
    assert!(!dce_modified, "DCE should not remove used value");
}
