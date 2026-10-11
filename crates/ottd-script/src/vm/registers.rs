//! Frame-relative register, literal and instruction-position operations.
use super::{Instruction, Value, Vm, VmError};

impl Vm<'_> {
    pub(super) fn register(&self, index: i32) -> Result<Value, VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        let index = self
            .base
            .checked_add(index)
            .ok_or(VmError::InvalidBytecode)?;
        if index >= self.top {
            return Err(VmError::InvalidBytecode);
        }
        self.registers
            .get(index)
            .cloned()
            .ok_or(VmError::InvalidBytecode)
    }
    pub(super) fn literal(&self, index: i32) -> Result<Value, VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        self.program()?
            .0
            .literals
            .get(index)
            .map(crate::table::scalar::Scalar::value)
            .ok_or(VmError::InvalidBytecode)
    }
    pub(super) fn write(&mut self, index: u8, value: Value) -> Result<(), VmError> {
        let index = self
            .base
            .checked_add(usize::from(index))
            .ok_or(VmError::InvalidBytecode)?;
        if index >= self.top {
            return Err(VmError::InvalidBytecode);
        }
        *self
            .registers
            .get_mut(index)
            .ok_or(VmError::InvalidBytecode)? = value;
        Ok(())
    }
    pub(super) fn jump(&mut self, offset: i32) -> Result<(), VmError> {
        let offset = isize::try_from(offset).map_err(|_| VmError::InvalidBytecode)?;
        let target = self
            .ip
            .checked_add_signed(offset)
            .ok_or(VmError::InvalidBytecode)?;
        if target >= self.program()?.0.instructions.len() {
            return Err(VmError::InvalidBytecode);
        }
        self.ip = target;
        Ok(())
    }
    pub(super) fn scope_end(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let from = self
            .base
            .checked_add(usize::from(instruction.arg0))
            .ok_or(VmError::InvalidBytecode)?;
        let count = instruction
            .arg1
            .checked_sub(i32::from(instruction.arg0))
            .and_then(|n| n.checked_add(2))
            .ok_or(VmError::InvalidBytecode)?;
        if from == 0 || from > self.top {
            return Err(VmError::InvalidBytecode);
        }
        // Nested-loop native last_stacksize may outlive inner local scopes.
        // A nonpositive signed cleanup count performs no stores in the native loop.
        if count <= 0 {
            return Ok(());
        }
        let count = usize::try_from(count).map_err(|_| VmError::InvalidBytecode)?;
        let end = from.checked_add(count).ok_or(VmError::InvalidBytecode)?;
        if from == 0 {
            return Err(VmError::InvalidBytecode);
        }
        if end <= self.top {
            self.registers
                .get_mut(from..end)
                .ok_or(VmError::InvalidBytecode)?
                .fill(Value::Null);
        }
        Ok(())
    }
    pub(super) fn return_value(&mut self, instruction: Instruction) -> Result<Value, VmError> {
        let value = if instruction.arg0 == 255 {
            Value::Null
        } else {
            self.register(instruction.arg1)?
        };
        self.store_temporary(value)
    }
}
