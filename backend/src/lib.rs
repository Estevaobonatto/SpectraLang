// Backend module - code generation using Cranelift

pub mod aot;
pub mod codegen;

pub use aot::AotCodeGenerator;
pub use codegen::CodeGenerator;
