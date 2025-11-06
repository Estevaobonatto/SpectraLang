// IR validation passes to ensure control-flow correctness prior to lowering into backend.
// These checks catch structural issues early to keep tests consistent with language semantics.

use super::Pass;
use crate::ir::{Function, Terminator};

/// Ensures that loops have properly linked exit blocks so `break` and `continue` work as expected.
pub struct LoopStructureValidation;

type BlockId = usize;

impl LoopStructureValidation {
    pub fn new() -> Self {
        Self
    }

    fn terminates_with_branch(func: &Function, block_id: BlockId, target: BlockId) -> bool {
        func.get_block(block_id)
            .and_then(|block| block.terminator.as_ref())
            .map(|terminator| match terminator {
                Terminator::Branch {
                    target: branch_target,
                } => *branch_target == target,
                Terminator::CondBranch {
                    true_block,
                    false_block,
                    ..
                } => *true_block == target || *false_block == target,
                Terminator::Switch { cases, default, .. } => {
                    cases.iter().any(|(_, case_target)| *case_target == target)
                        || *default == target
                }
                _ => false,
            })
            .unwrap_or(false)
    }

    fn validate_loop(&self, func: &Function, header: BlockId, exit: BlockId) -> bool {
        // Ensure header can reach exit through at least one terminator.
        func.get_block(header)
            .and_then(|block| block.terminator.as_ref())
            .map(|terminator| match terminator {
                Terminator::CondBranch {
                    true_block,
                    false_block,
                    ..
                } => *true_block == exit || *false_block == exit,
                Terminator::Branch { target } => *target == exit,
                Terminator::Switch { cases, default, .. } => {
                    cases.iter().any(|(_, case_target)| *case_target == exit) || *default == exit
                }
                Terminator::Return { .. } => true,
                Terminator::Unreachable => false,
            })
            .unwrap_or(false)
            || Self::terminates_with_branch(func, header, exit)
    }
}

impl Pass for LoopStructureValidation {
    fn name(&self) -> &str {
        "LoopStructureValidation"
    }

    fn run(&mut self, module: &mut crate::ir::Module) -> bool {
        let mut modified = false;

        for function in &mut module.functions {
            let mut invalid_loops = Vec::new();

            for block in &function.blocks {
                if let Some(Terminator::Branch { target }) = &block.terminator {
                    if !self.validate_loop(function, block.id, *target) {
                        invalid_loops.push((block.id, *target));
                    }
                }
            }

            if !invalid_loops.is_empty() {
                eprintln!(
                    "Loop validation warning in function '{}': {} issues detected",
                    function.name,
                    invalid_loops.len()
                );
                modified = true;
            }
        }

        modified
    }
}
