//! C++20 i64 bitwise operations with checked native shift counts.
use super::{Value, VmError};
impl Value {
    pub(crate) fn bitwise(self, rhs: Self, selector: u8) -> Result<Self, VmError> {
        if !matches!(selector, 0 | 2..=6) {
            return Err(VmError::InvalidBytecode);
        }
        let (Self::Integer(left), Self::Integer(right)) = (self, rhs) else {
            return Err(VmError::OperandType);
        };
        let result = match selector {
            0 => left & right,
            2 => left | right,
            3 => left ^ right,
            4..=6 => {
                let count = u32::try_from(right)
                    .ok()
                    .filter(|count| *count < 64)
                    .ok_or(VmError::UnsupportedShiftCount)?;
                let bits = u64::from_ne_bytes(left.to_ne_bytes());
                match selector {
                    4 => i64::from_ne_bytes((bits << count).to_ne_bytes()),
                    5 => left >> count,
                    _ => i64::from_ne_bytes((bits >> count).to_ne_bytes()),
                }
            }
            _ => return Err(VmError::InvalidBytecode),
        };
        Ok(Self::Integer(result))
    }
}
