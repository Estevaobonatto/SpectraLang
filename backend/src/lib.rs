// Backend module - code generation using Cranelift

pub mod aot;
pub mod codegen;
pub mod debug;
pub mod error;

pub use aot::{AotCodeGenerator, AotOptions};
pub use codegen::CodeGenerator;
pub use error::{BackendCodegenError, BackendErrorKind, BackendResult};
