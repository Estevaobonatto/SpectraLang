use std::collections::{HashMap, HashSet};

use crate::ir::{InstructionKind, Module, Terminator};

/// Performs structural verification of the IR and returns a list of problems if any were found.
pub fn verify_module(module: &Module) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    for function in &module.functions {
        if function.blocks.is_empty() {
            errors.push(format!(
                "Function '{}' has no basic blocks after lowering",
                function.name
            ));
            continue;
        }

        let block_ids: HashSet<usize> = function.blocks.iter().map(|block| block.id).collect();

        if block_ids.len() != function.blocks.len() {
            errors.push(format!(
                "Function '{}' contains duplicated block identifiers",
                function.name
            ));
        }

        for block in &function.blocks {
            if block.terminator.is_none() {
                errors.push(format!(
                    "Function '{}', block '{}' is missing a terminator",
                    function.name, block.label
                ));
            }

            if let Some(term) = &block.terminator {
                match term {
                    Terminator::Branch { target } => {
                        if !block_ids.contains(target) {
                            errors.push(format!(
                                "Function '{}', block '{}' branches to unknown block id {}",
                                function.name, block.label, target
                            ));
                        }
                    }
                    Terminator::Return { .. } => {}
                    Terminator::CondBranch {
                        true_block,
                        false_block,
                        ..
                    } => {
                        if !block_ids.contains(true_block) {
                            errors.push(format!(
                                "Function '{}', block '{}' has conditional branch with unknown true target {}",
                                function.name, block.label, true_block
                            ));
                        }
                        if !block_ids.contains(false_block) {
                            errors.push(format!(
                                "Function '{}', block '{}' has conditional branch with unknown false target {}",
                                function.name, block.label, false_block
                            ));
                        }
                    }
                    Terminator::Switch { cases, default, .. } => {
                        if !block_ids.contains(default) {
                            errors.push(format!(
                                "Function '{}', block '{}' has switch with unknown default target {}",
                                function.name, block.label, default
                            ));
                        }
                        for (_, target) in cases {
                            if !block_ids.contains(target) {
                                errors.push(format!(
                                    "Function '{}', block '{}' has switch with unknown case target {}",
                                    function.name, block.label, target
                                ));
                            }
                        }
                    }
                    Terminator::Unreachable => {}
                }
            }

            for instruction in &block.instructions {
                if let InstructionKind::Phi {
                    result: _,
                    incoming,
                } = &instruction.kind
                {
                    if incoming.is_empty() {
                        errors.push(format!(
                            "Function '{}', block '{}' contains phi with no incoming edges",
                            function.name, block.label
                        ));
                    }

                    let mut seen = HashMap::new();
                    for (value, pred) in incoming {
                        if !block_ids.contains(pred) {
                            errors.push(format!(
                                "Function '{}', block '{}' contains phi referencing unknown predecessor block {}",
                                function.name, block.label, pred
                            ));
                        }

                        if let Some(existing) = seen.insert(*pred, *value) {
                            errors.push(format!(
                                "Function '{}', block '{}' contains phi with duplicate entries for predecessor block {} (values {} and {})",
                                function.name,
                                block.label,
                                pred,
                                existing.id,
                                value.id
                            ));
                        }
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::IRBuilder;
    use crate::ir::{Function, Module as IRModule, Parameter, Terminator, Type, Value};

    #[test]
    fn detects_missing_terminator() {
        let mut module = IRModule::new("test");
        let mut function = Function::new(
            "foo",
            vec![Parameter {
                id: 0,
                name: "x".into(),
                ty: Type::Int,
            }],
            Type::Void,
        );

        let entry = function.add_block("entry");
        let mut builder = IRBuilder::new();
        builder.set_current_function(0);
        builder.set_current_block(entry);
        // Intentionally do not add terminator

        module.add_function(function);

        let result = verify_module(&module);
        assert!(result.is_err());
    }

    #[test]
    fn detects_unknown_branch_target() {
        let mut module = IRModule::new("test");
        let mut function = Function::new("foo", Vec::new(), Type::Void);
        let entry = function.add_block("entry");
        function.add_block("other");

        let mut builder = IRBuilder::new();
        builder.set_current_function(0);
        builder.set_current_block(entry);
        builder.build_branch(&mut function, 42);
        if let Some(block) = function.get_block_mut(other) {
            block.set_terminator(Terminator::Unreachable);
        }

        module.add_function(function);

        let result = verify_module(&module);
        assert!(result.is_err());
    }

    #[test]
    fn detects_phi_with_duplicate_predecessor() {
        let mut module = IRModule::new("test");
        let mut function = Function::new("foo", Vec::new(), Type::Void);
        let entry = function.add_block("entry");
        let other = function.add_block("other");

        let mut builder = IRBuilder::new();
        builder.set_current_function(0);
        builder.set_current_block(entry);

        let incoming = vec![(Value { id: 0 }, other), (Value { id: 1 }, other)];
        builder.build_phi(&mut function, incoming);
        builder.build_return(&mut function, None);
        if let Some(block) = function.get_block_mut(other) {
            block.set_terminator(Terminator::Unreachable);
        }

        module.add_function(function);
        let result = verify_module(&module);
        assert!(result.is_err());
    }

    #[test]
    fn passes_valid_module() {
        let mut module = IRModule::new("test");
        let mut function = Function::new("foo", Vec::new(), Type::Void);
        let entry = function.add_block("entry");
        let exit = function.add_block("exit");

        let mut builder = IRBuilder::new();
        builder.set_current_function(0);
        builder.set_current_block(entry);
        builder.build_branch(&mut function, exit);

        builder.set_current_block(exit);
        builder.build_return(&mut function, None);

        module.add_function(function);
        let result = verify_module(&module);
        assert!(result.is_ok());
    }
}
