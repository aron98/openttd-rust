//! Delete and clone publish their result without replacing the VM temporary.
use super::{Instruction, Value, Vm, VmError};
impl Vm<'_> {
    pub(super) fn clone_object(&self, i: Instruction) -> Result<Value, VmError> {
        if i.arg2 != 0 || i.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        match self.register(i.arg1)? {
            Value::Table(table) => table.shallow_clone().map(Value::Table),
            Value::Array(array) => array.shallow_clone().map(Value::Array),
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::NativeClosure(_)
            | Value::ScriptClosure(_) => Err(VmError::OperandType),
        }
    }
    pub(super) fn delete_slot(&self, i: Instruction) -> Result<Value, VmError> {
        if i.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        let key = self.register(i32::from(i.arg2))?;
        match self.register(i.arg1)? {
            Value::Table(table) => table.delete_slot(key),
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::NativeClosure(_)
            | Value::ScriptClosure(_) => Err(VmError::OperandType),
        }
    }
}
