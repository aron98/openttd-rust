//! Array iteration updates frame locals without replacing the runner temporary.
use super::{Instruction, Value, Vm, VmError};

impl Vm<'_> {
    pub(super) fn foreach(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let index = instruction.arg2;
        let value_register = index.checked_add(1).ok_or(VmError::InvalidBytecode)?;
        let cursor_register = index.checked_add(2).ok_or(VmError::InvalidBytecode)?;
        if instruction.arg3 != 0
            || instruction.arg0 == 0
            || index == 0
            || (index..=cursor_register).contains(&instruction.arg0)
            || usize::from(cursor_register) >= self.registers.len()
        {
            return Err(VmError::InvalidBytecode);
        }
        let array = match self.register(i32::from(instruction.arg0))? {
            Value::Array(array) => array,
            Value::String(_) => return Err(VmError::UnsupportedReceiver),
            Value::Null | Value::Bool(_) | Value::Integer(_) | Value::Float(_) => {
                return Err(VmError::OperandType);
            }
        };
        let cursor = match self.register(i32::from(cursor_register))? {
            Value::Null => 0,
            Value::Integer(cursor) => cursor,
            Value::Bool(_) | Value::Float(_) | Value::String(_) | Value::Array(_) => {
                return Err(VmError::InvalidBytecode);
            }
        };
        if let Some(value) = array.get(cursor) {
            let next = cursor.checked_add(1).ok_or(VmError::InvalidBytecode)?;
            self.jump(1)?;
            self.write(index, Value::Integer(cursor))?;
            self.write(value_register, value)?;
            self.write(cursor_register, Value::Integer(next))
        } else {
            self.jump(instruction.arg1)
        }
    }
}
