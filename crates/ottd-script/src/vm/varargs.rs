//! Unnamed argument ranges retain values independently of script registers.
use super::{Instruction, Value, Vm, VmError};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct VarArgs {
    // Native unsigned-short lookup offset; cleanup uses the physical stack length.
    pub base: usize,
    pub len: usize,
}

impl Vm<'_> {
    pub(super) fn vararg(&mut self, i: Instruction) -> Result<Value, VmError> {
        if i.arg2 != 0 || i.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        if i.opcode == 0x1c {
            if i.arg1 != 0 {
                return Err(VmError::InvalidBytecode);
            }
            return i64::try_from(self.varargs.len)
                .map(Value::Integer)
                .map_err(|_| VmError::InvalidBytecode);
        }
        let index = self.register(i.arg1)?;
        if self.varargs.len == 0 {
            return Err(
                self.native_failure(b"the function doesn't have var args", VmError::NativeCall)
            );
        }
        let index = match index {
            Value::Integer(index) => index,
            Value::Float(bits) => super::arrays::float_index(bits)?,
            Value::Null
            | Value::Bool(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Table(_)
            | Value::NativeClosure(_)
            | Value::ScriptClosure(_) => {
                let Value::String(kind) = index.type_name(&self.runner.get().root.as_ref().realm)
                else {
                    return Err(VmError::InvalidBytecode);
                };
                let mut message = b"indexing 'vargv' with ".to_vec();
                message.extend_from_slice(kind.as_bytes());
                return Err(self.native_failure(&message, VmError::NativeCall));
            }
        };
        let Ok(index) = usize::try_from(index) else {
            return Err(self.native_failure(b"vargv index out of range", VmError::NativeCall));
        };
        if index >= self.varargs.len {
            return Err(self.native_failure(b"vargv index out of range", VmError::NativeCall));
        }
        let index = self
            .varargs
            .base
            .checked_add(index)
            .ok_or(VmError::InvalidBytecode)?;
        self.varargs_stack
            .get(index)
            .cloned()
            .ok_or(VmError::InvalidBytecode)
    }
    pub(super) fn pop_varargs(&mut self) -> Result<(), VmError> {
        let retained = self
            .varargs_stack
            .len()
            .checked_sub(self.varargs.len)
            .ok_or(VmError::InvalidBytecode)?;
        self.varargs_stack.truncate(retained);
        self.varargs.len = 0;
        Ok(())
    }
}

#[cfg(test)]
mod owners;
#[cfg(test)]
mod safety;
