use std::fmt;

/// Compiler failure category, separate from execution failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileErrorKind {
    /// Syntax outside the currently supported scalar return grammar.
    UnsupportedSyntax,
    /// Malformed numeric literal.
    InvalidNumber,
    /// A required expression or delimiter is absent.
    ExpectedToken,
    /// Source or expression nesting exceeds the bounded compiler limits.
    Limit,
}
/// Source failure located by byte offset in the original UTF-8 input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompileError {
    /// Byte offset of the rejected token.
    pub offset: usize,
    /// Typed diagnostic category.
    pub kind: CompileErrorKind,
}
impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at byte {}", self.kind, self.offset)
    }
}
impl std::error::Error for CompileError {}

/// Checked execution failure; undefined native arithmetic remains unsupported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmError {
    /// Opcode has not been implemented by this foundation.
    UnsupportedOpcode(u8),
    /// Instruction operands, literal index or instruction pointer are invalid.
    InvalidBytecode,
    /// Operation requires a different scalar type.
    OperandType,
    /// Integer division by zero.
    DivisionByZero,
    /// Integer remainder by zero.
    ModuloByZero,
    /// Signed overflow has no portable native C++ semantics; compatibility is pending.
    UnsupportedOverflow,
    /// Budget credit would overflow the signed counter.
    BudgetOverflow,
    /// Execution has already returned or failed.
    Finished,
}
impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for VmError {}
