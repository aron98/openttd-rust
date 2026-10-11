//! Root identity is checked independently of scalar operand access.
use super::{Instruction, Temporary, Value, Vm, VmError};
impl Vm<'_> {
    pub(super) fn load_nulls(&mut self, i: Instruction) -> Result<(), VmError> {
        let length = usize::try_from(i.arg1).map_err(|_| VmError::InvalidBytecode)?;
        let start = usize::from(i.arg0);
        if start == 0 {
            return Err(VmError::InvalidBytecode);
        }
        let end = start.checked_add(length).ok_or(VmError::InvalidBytecode)?;
        self.registers
            .get_mut(start..end)
            .ok_or(VmError::InvalidBytecode)?
            .fill(Value::Null);
        Ok(())
    }
    pub(super) fn store_temporary(&mut self, value: Value) -> Result<Value, VmError> {
        self.runner.get_mut().temporary = Temporary::Value(value);
        match &self.runner.get().temporary {
            Temporary::Value(value) => Ok(value.clone()),
            Temporary::MainProgram { .. } => Err(VmError::UnsupportedRuntimeValue),
        }
    }
    pub(super) fn root_get(&mut self, instruction: Instruction) -> Result<Value, VmError> {
        let (receiver, key) = if instruction.opcode == 0x09 {
            (i32::from(instruction.arg2), self.literal(instruction.arg1)?)
        } else {
            (
                instruction.arg1,
                self.register(i32::from(instruction.arg2))?,
            )
        };
        let receiver = self.register(receiver)?;
        let current_this = receiver == self.register(0)?;
        let own = match &receiver {
            Value::Table(table) => table.lookup(key.clone()),
            Value::Array(array) => {
                let found = array.lookup(&key);
                if matches!(key, Value::Integer(_) | Value::Float(_)) {
                    return self.store_temporary(found?);
                }
                found
            }
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::NativeClosure(_) => {
                if !current_this {
                    return Err(VmError::UnsupportedReceiver);
                }
                this_scalar_lookup(&receiver, &key)
            }
        };
        let value = match own {
            Ok(value) => value,
            Err(VmError::MissingIndex) if current_this => self.configured_get(key)?,
            Err(error) => return Err(error),
        };
        self.store_temporary(value)
    }
    fn configured_get(&self, key: Value) -> Result<Value, VmError> {
        if let Some(value) = self.runner.get().root.table.raw_get(key.clone())? {
            return Ok(value);
        }
        if let Value::String(key) = key {
            match self.runner.get().root.realm.constant_bytes(key.as_bytes()) {
                Some(crate::realm::constants::Binding::Scalar(value)) => return Ok(value.value()),
                Some(crate::realm::constants::Binding::Enum(_)) => {
                    return Err(VmError::UnsupportedRuntimeValue);
                }
                None => {}
            }
        }
        Err(VmError::MissingIndex)
    }
    pub(super) fn root_store(&mut self, instruction: Instruction) -> Result<(), VmError> {
        if instruction.opcode == 0x3c && instruction.arg0 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        let key = self.register(i32::from(instruction.arg2))?;
        let value = self.register(i32::from(instruction.arg3))?;
        let newslot = matches!(instruction.opcode, 0x0b | 0x3c);
        if newslot && key == Value::Null {
            return Err(VmError::NullIndex);
        }
        let receiver = self.register(instruction.arg1)?;
        match &receiver {
            Value::Table(table) => {
                if newslot {
                    table.new_slot(key, value.clone())?;
                } else if let Err(error) = table.set_existing(key.clone(), value.clone()) {
                    if error == VmError::MissingIndex && receiver == self.register(0)? {
                        self.runner
                            .get()
                            .root
                            .table
                            .set_existing(key, value.clone())?;
                    } else {
                        return Err(error);
                    }
                }
            }
            Value::Array(array) => {
                if newslot {
                    return Err(VmError::OperandType);
                }
                array.store(&key, value.clone())?;
            }
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::NativeClosure(_) => return Err(VmError::UnsupportedReceiver),
        }
        if instruction.opcode != 0x3c && instruction.arg0 != instruction.arg3 {
            self.write(instruction.arg0, value)?;
        }
        Ok(())
    }
}

// SQVM::FallBackGet must reach a default delegate before the this/root fallback.
fn this_scalar_lookup(receiver: &Value, key: &Value) -> Result<Value, VmError> {
    if matches!(receiver, Value::String(_)) && matches!(key, Value::Integer(_) | Value::Float(_)) {
        return Err(VmError::UnsupportedReceiver);
    }
    let Value::String(key) = key else {
        return Err(VmError::MissingIndex);
    };
    let key = key.as_bytes();
    let method = match receiver {
        Value::String(_) => matches!(
            key,
            b"len"
                | b"tointeger"
                | b"tofloat"
                | b"tostring"
                | b"slice"
                | b"find"
                | b"tolower"
                | b"toupper"
                | b"weakref"
        ),
        Value::Integer(_) | Value::Float(_) | Value::Bool(_) => matches!(
            key,
            b"tointeger" | b"tofloat" | b"tostring" | b"tochar" | b"weakref"
        ),
        Value::NativeClosure(_) => matches!(
            key,
            b"call"
                | b"pcall"
                | b"acall"
                | b"pacall"
                | b"weakref"
                | b"tostring"
                | b"bindenv"
                | b"getinfos"
        ),
        Value::Null | Value::Array(_) | Value::Table(_) => false,
    };
    Err(if method {
        VmError::UnsupportedRuntimeValue
    } else {
        VmError::MissingIndex
    })
}
