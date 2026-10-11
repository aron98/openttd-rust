use std::fmt;

/// Reached native byte-classification context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCharacterContext {
    /// Default token classification in `SQLexer::Lex`.
    Token,
    /// Identifier continuation including its terminating lookahead.
    Identifier,
    /// Numeric prefix, digit, exponent or terminating lookahead.
    Number,
    /// Hexadecimal string escape digits including terminating lookahead.
    HexEscape,
}

/// Compiler failure category, separate from execution failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileErrorKind {
    /// Consumed malformed UTF-8 or a codepoint above native `MAX_CHAR` (0xffff).
    InvalidCharacter,
    /// Decoded codepoint outside the native byte ctype argument domain.
    UndefinedNativeCharacter {
        /// Decoded native codepoint (256..=65535).
        codepoint: u32,
        /// First reached classifier context.
        context: NativeCharacterContext,
    },
    /// Syntax outside the currently supported scalar statement grammar.
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
    /// An explicit script throw escaped every active exception trap.
    ThrownValue,
    /// A represented callable raised a native error retained by its runner.
    NativeCall,
    /// CALL reached a noncallable represented value.
    NotCallable,
    /// Target C++ standard-library string hash ABI is not admitted.
    UnsupportedTableHashTarget,
    /// Scalar table slots cannot yet retain aggregate graph edges.
    UnsupportedTableElement,
    /// Native pointer-based table ordering is not yet supported.
    UnsupportedTableOrdering,
    /// A new slot cannot have a null key.
    NullIndex,
    /// Native-valid aggregate elements are outside the scalar-array domain.
    UnsupportedArrayElement,
    /// Array ordering is outside the admitted identity-only comparison domain.
    UnsupportedArrayOrdering,
    /// A nonfinite or out-of-range f32 cannot be converted to a native integer index.
    UnsupportedIndexConversion,
    /// A reached failing closure tail call would consume native varargs twice.
    UndefinedNativeVarargsTailCall,
    /// Host element storage allocation failed; not a native semantic comparison.
    Allocation,
    /// Root belongs to a different string/constant realm.
    RealmMismatch,
    /// A required own or runtime-fallback slot is absent.
    MissingIndex,
    /// Lookup reached a native closure or aggregate outside the scalar domain.
    UnsupportedRuntimeValue,
    /// A native-valid receiver is outside the admitted operation domain.
    UnsupportedReceiver,
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
    /// A shift count is outside the defined native C++20 range 0..64.
    UnsupportedShiftCount,
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
