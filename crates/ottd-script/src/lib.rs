//! Bounded Squirrel source compilation and resumable register execution.
//!
//! This foundation deliberately rejects the unimplemented language and VM surface.
//! It does not yet execute AI/GS packages or connect to the game tick runtime.
mod array;
pub use array::Array;
mod compiler;
mod error;
mod lexer;
mod program;
mod realm;
mod runner;
pub use program::Program;
pub use realm::{ByteString, Realm};
pub use runner::{RootEnvironment, Runner, RunnerFailure};
mod value;
mod vm;
pub use compiler::{compile, compile_bytes};
pub use error::{CompileError, CompileErrorKind, NativeCharacterContext, VmError};
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

// Shared private owner types live at the crate boundary, without public re-exports.
#[derive(Debug)]
pub(crate) struct ProgramData {
    pub(crate) stack_size: u16,
    pub(crate) literals: Vec<Value>,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) realm: Realm,
}
#[derive(Debug)]
pub(crate) enum Temporary {
    Value(Value),
    MainProgram { _program: std::rc::Rc<ProgramData> },
}
#[derive(Debug)]
pub(crate) enum Storage<'a> {
    Owned(Runner),
    Borrowed(&'a mut Runner),
}
