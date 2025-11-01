// IR Builder - constructs IR from AST

use crate::ir::{Function, InstructionKind, Terminator, Value};

/// Builder for constructing IR
pub struct IRBuilder {
    current_function: Option<usize>,
    current_block: Option<usize>,
}

impl IRBuilder {
    pub fn new() -> Self {
        Self {
            current_function: None,
            current_block: None,
        }
    }

    pub fn set_current_function(&mut self, func_idx: usize) {
        self.current_function = Some(func_idx);
    }

    pub fn set_current_block(&mut self, block_id: usize) {
        self.current_block = Some(block_id);
    }

    pub fn get_current_block(&self) -> Option<usize> {
        self.current_block
    }

    pub fn build_add(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Add { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_sub(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Sub { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_mul(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Mul { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_div(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Div { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_rem(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Rem { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_eq(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Eq { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_ne(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Ne { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_lt(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Lt { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_le(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Le { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_gt(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Gt { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_ge(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Ge { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_and(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::And { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_or(&self, func: &mut Function, lhs: Value, rhs: Value) -> Value {
        let result = func.next_value();
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Or { result, lhs, rhs });
            }
        }
        result
    }

    pub fn build_return(&self, func: &mut Function, value: Option<Value>) {
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.set_terminator(Terminator::Return { value });
            }
        }
    }

    pub fn build_branch(&self, func: &mut Function, target: usize) {
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.set_terminator(Terminator::Branch { target });
            }
        }
    }

    pub fn build_cond_branch(
        &self,
        func: &mut Function,
        condition: Value,
        true_block: usize,
        false_block: usize,
    ) {
        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.set_terminator(Terminator::CondBranch {
                    condition,
                    true_block,
                    false_block,
                });
            }
        }
    }

    pub fn build_call(
        &self,
        func: &mut Function,
        function_name: String,
        args: Vec<Value>,
        has_return: bool,
    ) -> Option<Value> {
        let result = if has_return {
            Some(func.next_value())
        } else {
            None
        };

        if let Some(block_id) = self.current_block {
            if let Some(block) = func.get_block_mut(block_id) {
                block.add_instruction(InstructionKind::Call {
                    result,
                    function: function_name,
                    args,
                });
            }
        }

        result
    }
}

impl Default for IRBuilder {
    fn default() -> Self {
        Self::new()
    }
}
