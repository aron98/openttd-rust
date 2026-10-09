use crate::VmError;

/// Scalar value; float bits preserve negative zero and nonfinite payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    /// The null singleton.
    Null,
    /// Signed 64-bit Squirrel integer.
    Integer(i64),
    /// IEEE 754 single precision raw bits.
    Float(u32),
    /// Boolean singleton.
    Bool(bool),
}
impl Value {
    pub(crate) const fn is_false(self) -> bool {
        match self {
            Self::Null => true,
            Self::Integer(n) => n == 0,
            Self::Float(bits) => bits.trailing_zeros() >= 31,
            Self::Bool(b) => !b,
        }
    }
    pub(crate) fn negate(self) -> Result<Self, VmError> {
        match self {
            Self::Integer(n) => n
                .checked_neg()
                .map(Self::Integer)
                .ok_or(VmError::UnsupportedOverflow),
            Self::Float(bits) => Ok(Self::Float(bits ^ 0x8000_0000)),
            Self::Null | Self::Bool(_) => Err(VmError::OperandType),
        }
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "Pinned Squirrel explicitly converts i64 operands to SQFloat (f32)"
    )]
    const fn numeric(self) -> Result<f32, VmError> {
        match self {
            Self::Integer(n) => Ok(n as f32),
            Self::Float(bits) => Ok(f32::from_bits(bits)),
            Self::Null | Self::Bool(_) => Err(VmError::OperandType),
        }
    }
    pub(crate) fn arithmetic(self, other: Self, op: u8) -> Result<Self, VmError> {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => {
                let result = match op {
                    b'+' => left.checked_add(right),
                    b'-' => left.checked_sub(right),
                    b'*' => left.checked_mul(right),
                    b'/' => {
                        if right == 0 {
                            return Err(VmError::DivisionByZero);
                        }
                        left.checked_div(right)
                    }
                    b'%' => {
                        if right == 0 {
                            return Err(VmError::ModuloByZero);
                        }
                        left.checked_rem(right)
                    }
                    _ => return Err(VmError::InvalidBytecode),
                };
                result
                    .map(Self::Integer)
                    .ok_or(VmError::UnsupportedOverflow)
            }
            (Self::Integer(_) | Self::Float(_), Self::Integer(_) | Self::Float(_)) => {
                float_arithmetic(self.numeric()?, other.numeric()?, op)
                    .map(|n| Self::Float(n.to_bits()))
            }
            (Self::Null | Self::Bool(_), _) | (_, Self::Null | Self::Bool(_)) => {
                Err(VmError::OperandType)
            }
        }
    }
}
fn float_arithmetic(left: f32, right: f32, op: u8) -> Result<f32, VmError> {
    match op {
        b'+' => Ok(left + right),
        b'-' => Ok(left - right),
        b'*' => Ok(left * right),
        b'/' => Ok(left / right),
        b'%' => Ok(left % right),
        _ => Err(VmError::InvalidBytecode),
    }
}
