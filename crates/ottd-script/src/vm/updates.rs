//! Update stores follow native `LOCAL_INC/PLOCAL_INC` sequencing, including aliases.
use super::{Instruction, Value, Vm, VmError};
impl Vm<'_> {
    pub(super) fn update(&mut self, instruction: Instruction) -> Result<(), VmError> {
        let source = self.register(instruction.arg1)?;
        let increment = if instruction.opcode == 0x23 {
            self.register(i32::from(instruction.arg2))?
        } else {
            Value::Integer(i64::from(i8::from_ne_bytes([instruction.arg3])))
        };
        let operation = if instruction.opcode == 0x23 {
            instruction.arg3
        } else {
            b'+'
        };
        let result = source.arithmetic(&increment, operation, &self.runner.get().root.realm)?;
        let register = u8::try_from(instruction.arg1).map_err(|_| VmError::InvalidBytecode)?;
        self.write(
            instruction.arg0,
            if instruction.opcode == 0x27 {
                source
            } else {
                result.clone()
            },
        )?;
        self.write(register, result)
    }
}
impl Vm<'_> {
    pub(super) fn compound_field(&mut self, i: Instruction) -> Result<(), VmError> {
        let receiver = self.register((i.arg1 >> 16) & 0xffff)?;
        let key = self.register(i32::from(i.arg2))?;
        let source = self.lookup_value(&receiver, key.clone())?;
        let increment = self.register(i.arg1 & 0xffff)?;
        let value = source.arithmetic(&increment, i.arg3, &self.runner.get().root.realm)?;
        self.write(i.arg0, value.clone())?;
        let current_this = receiver == self.register(0)?;
        let stored = match &receiver {
            Value::Table(table) => match table.set_existing(key.clone(), value.clone()) {
                Err(VmError::MissingIndex) if current_this => {
                    self.runner.get().root.table.set_existing(key, value)
                }
                result => result,
            },
            Value::Array(array) => array.store(&key, value),
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::NativeClosure(_)
            | Value::ScriptClosure(_) => {
                let Value::String(kind) = receiver.type_name(&self.runner.get().root.realm) else {
                    return Err(VmError::InvalidBytecode);
                };
                let mut message = b"trying to set '".to_vec();
                message.extend_from_slice(kind.as_bytes());
                message.push(b'\'');
                let _error = self.native_failure(&message, VmError::OperandType);
                return Ok(());
            }
        };
        // Native DerefInc deliberately ignores Set's false result after storing
        // the arithmetic target; an absent table slot does not raise an error.
        match stored {
            Ok(()) | Err(VmError::MissingIndex) => Ok(()),
            Err(error) => Err(error),
        }
    }
}
