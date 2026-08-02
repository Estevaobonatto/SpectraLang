// Backend module - code generation using Cranelift

pub mod aot;
pub mod codegen;
pub mod debug;
pub mod dwarf;
pub mod error;
pub(crate) mod hostcall_abi;

pub use aot::{AotCodeGenerator, AotOptions};
pub use codegen::{CodeGenerator, HostCallBatchStats};
pub use error::{BackendCodegenError, BackendErrorKind, BackendResult};
