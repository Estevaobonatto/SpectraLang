use std::collections::{HashMap, HashSet};

use crate::ir::{Instruction, InstructionKind, Module, Terminator, Value};

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
        let mut defined_values: HashSet<usize> =
            function.params.iter().map(|param| param.id).collect();

        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(result) = instruction_result(instruction) {
                    if !defined_values.insert(result.id) {
                        errors.push(format!(
                            "Function '{}' defines value {} more than once",
                            function.name, result.id
                        ));
                    }
                }
            }
        }

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
                    Terminator::Return { value } => {
                        if let Some(value) = value {
                            check_value_defined(
                                &mut errors,
                                &function.name,
                                &block.label,
                                *value,
                                &defined_values,
                            );
                        }
                    }
                    Terminator::CondBranch {
                        condition,
                        true_block,
                        false_block,
                        ..
                    } => {
                        check_value_defined(
                            &mut errors,
                            &function.name,
                            &block.label,
                            *condition,
                            &defined_values,
                        );
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
                    Terminator::Switch {
                        value,
                        cases,
                        default,
                    } => {
                        check_value_defined(
                            &mut errors,
                            &function.name,
                            &block.label,
                            *value,
                            &defined_values,
                        );
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
                for operand in instruction_operands(instruction) {
                    check_value_defined(
                        &mut errors,
                        &function.name,
                        &block.label,
                        operand,
                        &defined_values,
                    );
                }

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

fn check_value_defined(
    errors: &mut Vec<String>,
    function_name: &str,
    block_label: &str,
    value: Value,
    defined_values: &HashSet<usize>,
) {
    if !defined_values.contains(&value.id) {
        errors.push(format!(
            "Function '{}', block '{}' uses undefined value {}",
            function_name, block_label, value.id
        ));
    }
}

fn instruction_result(instruction: &Instruction) -> Option<Value> {
    match &instruction.kind {
        InstructionKind::Add { result, .. }
        | InstructionKind::Sub { result, .. }
        | InstructionKind::Mul { result, .. }
        | InstructionKind::Div { result, .. }
        | InstructionKind::Rem { result, .. }
        | InstructionKind::Eq { result, .. }
        | InstructionKind::Ne { result, .. }
        | InstructionKind::Lt { result, .. }
        | InstructionKind::Le { result, .. }
        | InstructionKind::Gt { result, .. }
        | InstructionKind::Ge { result, .. }
        | InstructionKind::And { result, .. }
        | InstructionKind::Or { result, .. }
        | InstructionKind::Not { result, .. }
        | InstructionKind::Alloca { result, .. }
        | InstructionKind::Load { result, .. }
        | InstructionKind::GetElementPtr { result, .. }
        | InstructionKind::FuncAddr { result, .. }
        | InstructionKind::AsyncReady { result, .. }
        | InstructionKind::Phi { result, .. }
        | InstructionKind::Copy { result, .. }
        | InstructionKind::ConstInt { result, .. }
        | InstructionKind::ConstFloat { result, .. }
        | InstructionKind::ConstBool { result, .. }
        | InstructionKind::Cast { result, .. }
        | InstructionKind::MakeDynFatPtr { result, .. }
        | InstructionKind::LoadDynDataPtr { result, .. }
        | InstructionKind::LoadDynVtablePtr { result, .. }
        | InstructionKind::LoadVtableSlot { result, .. } => Some(*result),
        InstructionKind::Call { result, .. }
        | InstructionKind::HostCall { result, .. }
        | InstructionKind::CallIndirect { result, .. } => *result,
        InstructionKind::Store { .. }
        | InstructionKind::AsyncSuspend { .. }
        | InstructionKind::AsyncResume { .. } => None,
    }
}

fn instruction_operands(instruction: &Instruction) -> Vec<Value> {
    match &instruction.kind {
        InstructionKind::Add { lhs, rhs, .. }
        | InstructionKind::Sub { lhs, rhs, .. }
        | InstructionKind::Mul { lhs, rhs, .. }
        | InstructionKind::Div { lhs, rhs, .. }
        | InstructionKind::Rem { lhs, rhs, .. }
        | InstructionKind::Eq { lhs, rhs, .. }
        | InstructionKind::Ne { lhs, rhs, .. }
        | InstructionKind::Lt { lhs, rhs, .. }
        | InstructionKind::Le { lhs, rhs, .. }
        | InstructionKind::Gt { lhs, rhs, .. }
        | InstructionKind::Ge { lhs, rhs, .. }
        | InstructionKind::And { lhs, rhs, .. }
        | InstructionKind::Or { lhs, rhs, .. } => vec![*lhs, *rhs],
        InstructionKind::Not { operand, .. }
        | InstructionKind::Load { ptr: operand, .. }
        | InstructionKind::Copy {
            source: operand, ..
        }
        | InstructionKind::Cast { operand, .. }
        | InstructionKind::LoadDynDataPtr {
            fat_ptr: operand, ..
        }
        | InstructionKind::LoadDynVtablePtr {
            fat_ptr: operand, ..
        }
        | InstructionKind::LoadVtableSlot {
            vtable_ptr: operand,
            ..
        }
        | InstructionKind::AsyncSuspend { task: operand, .. }
        | InstructionKind::AsyncResume { task: operand, .. } => vec![*operand],
        InstructionKind::Store { ptr, value } => vec![*ptr, *value],
        InstructionKind::GetElementPtr { ptr, index, .. } => vec![*ptr, *index],
        InstructionKind::Call { args, .. } | InstructionKind::HostCall { args, .. } => args.clone(),
        InstructionKind::CallIndirect { fn_ptr, args, .. } => {
            let mut operands = Vec::with_capacity(args.len() + 1);
            operands.push(*fn_ptr);
            operands.extend(args.iter().copied());
            operands
        }
        InstructionKind::AsyncReady { value, .. } => value.iter().copied().collect(),
        InstructionKind::Phi { incoming, .. } => incoming.iter().map(|(value, _)| *value).collect(),
        InstructionKind::MakeDynFatPtr {
            data_ptr,
            vtable_ptr,
            ..
        } => vec![*data_ptr, *vtable_ptr],
        InstructionKind::Alloca { .. }
        | InstructionKind::FuncAddr { .. }
        | InstructionKind::ConstInt { .. }
        | InstructionKind::ConstFloat { .. }
        | InstructionKind::ConstBool { .. } => Vec::new(),
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
        let other = function.add_block("other");

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
    fn detects_undefined_instruction_operand() {
        let mut module = IRModule::new("test");
        let mut function = Function::new("foo", Vec::new(), Type::Void);
        let entry = function.add_block("entry");

        let mut builder = IRBuilder::new();
        builder.set_current_function(0);
        builder.set_current_block(entry);
        let slot = builder.build_alloca(&mut function, Type::Int);
        builder.build_store(&mut function, slot, Value { id: 13 });
        builder.build_return(&mut function, None);

        module.add_function(function);
        let result = verify_module(&module).expect_err("undefined value must fail verification");
        assert!(result
            .iter()
            .any(|error| error.contains("uses undefined value 13")));
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
