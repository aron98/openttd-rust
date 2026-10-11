//! Native raw Get for EXISTS bypasses default methods and preserves misses' temporary.
use super::{Instruction, Value, Vm, VmError, arrays::float_index};

impl Vm<'_> {
    pub(super) fn membership(&mut self, instruction: Instruction) -> Result<Value, VmError> {
        if instruction.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        let receiver = self.register(instruction.arg1)?;
        let key = self.register(i32::from(instruction.arg2))?;
        let value = match receiver {
            Value::Table(table) => table.raw_get(key)?,
            Value::Array(array) => numeric_index(&key)?.and_then(|index| array.get(index)),
            Value::String(string) => {
                let length = i64::try_from(string.as_bytes().len())
                    .map_err(|_| VmError::UnsupportedIndexConversion)?;
                numeric_index(&key)?.and_then(|index| {
                    let index = if index < 0 {
                        length.checked_add(index)?
                    } else {
                        index
                    };
                    string
                        .as_bytes()
                        .get(usize::try_from(index).ok()?)
                        .map(|byte| {
                            Value::Integer(i64::from(std::ffi::c_char::from_ne_bytes([*byte])))
                        })
                })
            }
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::NativeClosure(_)
            | Value::ScriptClosure(_) => None,
        };
        if let Some(value) = value {
            let _value = self.store_temporary(value)?;
            Ok(Value::Bool(true))
        } else {
            Ok(Value::Bool(false))
        }
    }
}

fn numeric_index(value: &Value) -> Result<Option<i64>, VmError> {
    match value {
        Value::Integer(index) => Ok(Some(*index)),
        Value::Float(bits) => float_index(*bits).map(Some),
        Value::Null
        | Value::Bool(_)
        | Value::String(_)
        | Value::Array(_)
        | Value::Table(_)
        | Value::NativeClosure(_)
        | Value::ScriptClosure(_) => Ok(None),
    }
}
