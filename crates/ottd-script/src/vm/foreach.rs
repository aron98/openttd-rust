//! Array, table and byte-string iteration updates frame locals without replacing the runner temporary.
use super::{Instruction, Value, Vm, VmError};

impl Vm<'_> {
    pub(super) fn foreach(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let index = instruction.arg2;
        let value_register = index.checked_add(1).ok_or(VmError::InvalidBytecode)?;
        let cursor_register = index.checked_add(2).ok_or(VmError::InvalidBytecode)?;
        if instruction.arg3 != 0
            || index == 0
            || (index..=cursor_register).contains(&instruction.arg0)
            || usize::from(cursor_register) >= self.registers.len()
        {
            return Err(VmError::InvalidBytecode);
        }
        let receiver = self.register(i32::from(instruction.arg0))?;
        match &receiver {
            Value::Array(_) | Value::Table(_) | Value::String(_) => {}
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::NativeClosure(_) => {
                return Err(VmError::OperandType);
            }
        }
        let cursor = match self.register(i32::from(cursor_register))? {
            Value::Null => 0,
            Value::Integer(cursor) => cursor,
            Value::Bool(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Table(_)
            | Value::NativeClosure(_) => {
                return Err(VmError::InvalidBytecode);
            }
        };
        if let Value::Table(table) = &receiver {
            let cursor = usize::try_from(cursor).map_err(|_| VmError::InvalidBytecode)?;
            if let Some((next, key, value)) = table.next(cursor) {
                self.jump(1)?;
                self.write(index, key)?;
                self.write(value_register, value)?;
                return self.write(
                    cursor_register,
                    Value::Integer(i64::try_from(next).map_err(|_| VmError::InvalidBytecode)?),
                );
            }
            return self.jump(instruction.arg1);
        }
        let value = match receiver {
            Value::Table(_) => return Err(VmError::InvalidBytecode),
            Value::Array(array) => array.get(cursor),
            Value::String(string) => {
                // Native string Next widens target C char; negative cursors are native UB.
                let index = usize::try_from(cursor).map_err(|_| VmError::InvalidBytecode)?;
                string
                    .as_bytes()
                    .get(index)
                    .map(|byte| Value::Integer(i64::from(std::ffi::c_char::from_ne_bytes([*byte]))))
            }
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::NativeClosure(_) => {
                return Err(VmError::OperandType);
            }
        };
        if let Some(value) = value {
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

#[cfg(test)]
mod tests;
