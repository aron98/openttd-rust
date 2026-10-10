//! Scalar Squirrel source compilation and resumable register execution.
//!
//! This foundation deliberately rejects the unimplemented language and VM surface.
//! It does not yet execute AI/GS packages or connect to the game tick runtime.
mod compiler;
mod error;
mod lexer;
mod value;
mod vm;
pub use compiler::{compile, compile_bytes};
pub use error::{CompileError, CompileErrorKind, VmError};
pub use value::Value;
pub use vm::{Execution, Vm};

/// Native Squirrel instruction tuple, with an i32 immediate and three u8 operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// Native opcode number.
    pub opcode: u8,
    /// Destination register or instruction flag.
    pub arg0: u8,
    /// Immediate, literal index or source register.
    pub arg1: i32,
    /// Second register operand.
    pub arg2: u8,
    /// Operation selector or second literal index.
    pub arg3: u8,
}

/// Compiled main function. Register zero is reserved for the root environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// Number of registers required by the function.
    pub stack_size: u16,
    /// Constant pool in compiler encounter order.
    pub literals: Vec<Value>,
    /// Native instruction stream, including the implicit trailing return.
    pub instructions: Vec<Instruction>,
}
