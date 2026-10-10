//! Collection operations share the existing instruction loop and temporary owner.
use super::{Instruction, Slot, Value, Vm, VmError};
use crate::Array;
impl Vm<'_> {
    pub(super) fn new_array(&self, instruction: Instruction) -> Result<Value, VmError> {
        if instruction.arg0 == 0 || instruction.arg2 != 0 || instruction.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        self.registers
            .get(usize::from(instruction.arg0))
            .ok_or(VmError::InvalidBytecode)?;
        let capacity = usize::try_from(instruction.arg1).map_err(|_| VmError::InvalidBytecode)?;
        Array::reserved(&self.runner.get().root.realm, capacity).map(Value::Array)
    }
    pub(super) fn append_array(&self, instruction: Instruction) -> Result<(), VmError> {
        let Slot::Value(Value::Array(array)) = self
            .registers
            .get(usize::from(instruction.arg0))
            .ok_or(VmError::InvalidBytecode)?
        else {
            return Err(VmError::InvalidBytecode);
        };
        let value = match (instruction.arg2, instruction.arg3) {
            (0, 0) => self.register(instruction.arg1)?,
            (255, 255) => self.literal(instruction.arg1)?,
            _ => return Err(VmError::InvalidBytecode),
        };
        array.append(value)
    }
}
impl Array {
    pub(crate) fn lookup(&self, key: &Value) -> Result<Value, VmError> {
        match key {
            Value::Integer(index) => self.get(*index).ok_or(VmError::MissingIndex),
            Value::Float(bits) => self.get(float_index(*bits)?).ok_or(VmError::MissingIndex),
            Value::String(key)
                if matches!(
                    key.as_bytes(),
                    b"len"
                        | b"append"
                        | b"extend"
                        | b"push"
                        | b"pop"
                        | b"top"
                        | b"insert"
                        | b"remove"
                        | b"resize"
                        | b"reverse"
                        | b"sort"
                        | b"slice"
                        | b"weakref"
                        | b"tostring"
                        | b"clear"
                ) =>
            {
                Err(VmError::UnsupportedRuntimeValue)
            }
            Value::String(_) | Value::Null | Value::Bool(_) | Value::Array(_) => {
                Err(VmError::MissingIndex)
            }
        }
    }
    pub(crate) fn store(&self, key: &Value, value: Value) -> Result<(), VmError> {
        let index = match key {
            Value::Integer(index) => *index,
            Value::Float(bits) => float_index(*bits)?,
            Value::Null | Value::Bool(_) | Value::String(_) | Value::Array(_) => {
                return Err(VmError::OperandType);
            }
        };
        self.set(index, value)
    }
}
fn float_index(bits: u32) -> Result<i64, VmError> {
    let exponent = (bits >> 23) & 255;
    if exponent == 255 || exponent > 190 {
        return Err(VmError::UnsupportedIndexConversion);
    }
    if exponent < 127 {
        return Ok(0);
    }
    let significand = u64::from(bits & 0x7f_ffff) | (1_u64 << 23);
    let magnitude = if exponent >= 150 {
        significand.checked_shl(exponent.saturating_sub(150))
    } else {
        significand.checked_shr(150_u32.saturating_sub(exponent))
    }
    .ok_or(VmError::UnsupportedIndexConversion)?;
    if bits >> 31 != 0 && magnitude == (1_u64 << 63) {
        return Ok(i64::MIN);
    }
    let value = i64::try_from(magnitude).map_err(|_| VmError::UnsupportedIndexConversion)?;
    if bits >> 31 == 0 {
        Ok(value)
    } else {
        value
            .checked_neg()
            .ok_or(VmError::UnsupportedIndexConversion)
    }
}
