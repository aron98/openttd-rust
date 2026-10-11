//! Delegate installation and parent results preserve the original table identity.
use super::{Instruction, Value, Vm, VmError};

impl Vm<'_> {
    pub(super) fn attach_delegate(&mut self, i: Instruction) -> Result<Value, VmError> {
        if i.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        let receiver = self.register(i.arg1)?;
        let delegate = self.register(i32::from(i.arg2))?;
        let Value::Table(table) = &receiver else {
            let message = format!("delegating a '{}'", receiver.type_label());
            return Err(self.native_failure(message.as_bytes(), VmError::OperandType));
        };
        let delegate = match &delegate {
            Value::Table(table) => Some(table),
            Value::Null => None,
            Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::NativeClosure(_)
            | Value::ScriptClosure(_) => {
                let message = format!("using '{}' as delegate", delegate.type_label());
                return Err(self.native_failure(message.as_bytes(), VmError::OperandType));
            }
        };
        match table.set_delegate(delegate) {
            Err(VmError::DelegateCycle) => {
                Err(self.native_failure(b"delegate cycle detected", VmError::DelegateCycle))
            }
            Err(error) => Err(error),
            Ok(()) => Ok(receiver),
        }
    }
    pub(super) fn parent_value(&mut self, i: Instruction) -> Result<Value, VmError> {
        if i.arg2 != 0 || i.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        let receiver = self.register(i.arg1)?;
        if let Value::Table(table) = receiver {
            Ok(table.delegate().map_or(Value::Null, Value::Table))
        } else {
            let message = format!(
                "the {} type doesn't have a parent slot",
                receiver.type_label()
            );
            Err(self.native_failure(message.as_bytes(), VmError::OperandType))
        }
    }
}
