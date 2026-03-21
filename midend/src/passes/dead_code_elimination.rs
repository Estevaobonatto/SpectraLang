// Dead code elimination pass
// Removes instructions whose results are never used

use crate::ir::{InstructionKind, Module, Terminator};
use crate::passes::Pass;
use std::collections::HashSet;

pub struct DeadCodeElimination;

impl DeadCodeElimination {
    pub fn new() -> Self {
        Self
    }
}

impl Pass for DeadCodeElimination {
    fn name(&self) -> &str {
        "DeadCodeElimination"
    }

    fn run(&mut self, module: &mut Module) -> bool {
        let mut modified = false;

        for func in &mut module.functions {
            let mut used_values: HashSet<usize> = HashSet::new();

            // Mark values used in terminators
            for block in &func.blocks {
                if let Some(terminator) = &block.terminator {
                    match terminator {
                        Terminator::Return { value: Some(v) } => {
                            used_values.insert(v.id);
                        }
                        Terminator::CondBranch { condition, .. } => {
                            used_values.insert(condition.id);
                        }
                        Terminator::Switch { value, .. } => {
                            used_values.insert(value.id);
                        }
                        _ => {}
                    }
                }

                // Mark values used in instructions
                for instr in &block.instructions {
                    Self::mark_used_values(&instr.kind, &mut used_values);
                }
            }

            // Collect dead instructions (instructions whose results are never used)
            let mut dead_instructions: Vec<(usize, usize)> = Vec::new();

            for (block_idx, block) in func.blocks.iter().enumerate() {
                for (instr_idx, instr) in block.instructions.iter().enumerate() {
                    if let Some(result_id) = Self::get_result_id(&instr.kind) {
                        if !used_values.contains(&result_id) && !Self::has_side_effects(&instr.kind)
                        {
                            dead_instructions.push((block_idx, instr_idx));
                        }
                    }
                }
            }

            // Remove dead instructions (iterate in reverse to preserve indices)
            for (block_idx, instr_idx) in dead_instructions.into_iter().rev() {
                if let Some(block) = func.blocks.get_mut(block_idx) {
                    block.instructions.remove(instr_idx);
                    modified = true;
                }
            }
        }

        modified
    }
}

impl DeadCodeElimination {
    fn mark_used_values(instr: &InstructionKind, used: &mut HashSet<usize>) {
        match instr {
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
            | InstructionKind::Or { lhs, rhs, .. } => {
                used.insert(lhs.id);
                used.insert(rhs.id);
            }
            InstructionKind::Not { operand, .. } => {
                used.insert(operand.id);
            }
            InstructionKind::Load { ptr, .. } => {
                used.insert(ptr.id);
            }
            InstructionKind::Store { ptr, value } => {
                used.insert(ptr.id);
                used.insert(value.id);
            }
            InstructionKind::GetElementPtr { ptr, index, .. } => {
                used.insert(ptr.id);
                used.insert(index.id);
            }
            InstructionKind::Call { args, .. } => {
                for arg in args {
                    used.insert(arg.id);
                }
            }
            InstructionKind::Copy { source, .. } => {
                used.insert(source.id);
            }
            InstructionKind::Phi { incoming, .. } => {
                for (value, _) in incoming {
                    used.insert(value.id);
                }
            }
            _ => {}
        }
    }

    fn get_result_id(instr: &InstructionKind) -> Option<usize> {
        match instr {
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
            | InstructionKind::Copy { result, .. }
            | InstructionKind::Phi { result, .. }
            | InstructionKind::ConstInt { result, .. }
            | InstructionKind::ConstFloat { result, .. }
            | InstructionKind::ConstBool { result, .. } => Some(result.id),
            InstructionKind::Call { result, .. } => result.as_ref().map(|r| r.id),
            _ => None,
        }
    }

    fn has_side_effects(instr: &InstructionKind) -> bool {
        matches!(
            instr,
            InstructionKind::Store { .. } | InstructionKind::Call { .. }
        )
    }
}

impl Default for DeadCodeElimination {
    fn default() -> Self {
        Self::new()
    }
}

/// Free helper — cria e executa o passo em uma única chamada.
pub fn run(module: &mut crate::ir::Module) -> bool {
    DeadCodeElimination::new().run(module)
}
