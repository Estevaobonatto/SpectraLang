// Spectra Intermediate Representation (SIR)
// SSA-based IR for optimization and code generation

pub mod builder;
pub mod ir;
pub mod lowering;
pub mod passes;

pub use builder::IRBuilder;
pub use ir::{
    BasicBlock, Function as IRFunction, Instruction, Module as IRModule, Type as IRType, Value,
};
pub use lowering::ASTLowering;
