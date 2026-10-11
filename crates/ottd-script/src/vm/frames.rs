//! Script calls install frames in the existing dispatch loop.
use super::{Instruction, Value, Vm, VmError};
use crate::ScriptClosure;
#[derive(Debug)]
pub(super) struct ParentFrame {
    closure: ScriptClosure,
    base: usize,
    top: usize,
    ip: usize,
    target: u8,
}
impl Vm<'_> {
    pub(super) fn dispatch_call(&mut self, i: Instruction) -> Result<(), VmError> {
        let callee = self.store_temporary(self.register(i.arg1)?)?;
        match callee {
            Value::ScriptClosure(closure) => self.script_call(i, closure),
            Value::NativeClosure(closure) => {
                let result = self.call(i, closure)?;
                self.write(i.arg0, result)
            }
            other => {
                let Value::String(kind) = other.type_name(&self.runner.get().root.realm) else {
                    return Err(VmError::InvalidBytecode);
                };
                let mut message = b"attempt to call '".to_vec();
                message.extend_from_slice(kind.as_bytes());
                message.push(b'\'');
                Err(self.native_failure(&message, VmError::NotCallable))
            }
        }
    }
    fn script_call(&mut self, i: Instruction, closure: ScriptClosure) -> Result<(), VmError> {
        let tail = i.opcode == 0x05;
        if i.arg3 == 0 {
            return Err(VmError::InvalidBytecode);
        }
        let argument_base = self
            .base
            .checked_add(usize::from(i.arg2))
            .ok_or(VmError::InvalidBytecode)?;
        let argument_end = argument_base
            .checked_add(usize::from(i.arg3))
            .ok_or(VmError::InvalidBytecode)?;
        if argument_end > self.top {
            return Err(VmError::InvalidBytecode);
        }
        if tail {
            for offset in 0..usize::from(i.arg3) {
                let source = argument_base
                    .checked_add(offset)
                    .ok_or(VmError::InvalidBytecode)?;
                let destination = self
                    .base
                    .checked_add(offset)
                    .ok_or(VmError::InvalidBytecode)?;
                let value = self
                    .registers
                    .get(source)
                    .cloned()
                    .ok_or(VmError::InvalidBytecode)?;
                *self
                    .registers
                    .get_mut(destination)
                    .ok_or(VmError::InvalidBytecode)? = value;
            }
        }
        let base = if tail { self.base } else { argument_base };
        let proto = closure.prototype();
        let count = usize::from(i.arg3);
        let missing = proto.0.parameters.checked_sub(count);
        if count != proto.0.parameters
            && missing.is_none_or(|missing| missing > proto.0.defaults.len())
        {
            return Err(self.native_failure(b"wrong number of parameters", VmError::NativeCall));
        }
        let top = base
            .checked_add(usize::from(proto.0.stack_size))
            .ok_or(VmError::InvalidBytecode)?;
        self.grow_stack(top)?;
        if let Some(missing) = missing {
            for offset in 0..missing {
                let index = proto
                    .0
                    .defaults
                    .len()
                    .checked_sub(missing)
                    .and_then(|index| index.checked_add(offset))
                    .ok_or(VmError::InvalidBytecode)?;
                let value = closure
                    .read(|data| {
                        data.defaults
                            .get(index)
                            .map(|value| value.value(&closure.0.realm))
                    })
                    .ok_or(VmError::InvalidBytecode)?;
                let slot = base
                    .checked_add(count)
                    .and_then(|slot| slot.checked_add(offset))
                    .ok_or(VmError::InvalidBytecode)?;
                *self
                    .registers
                    .get_mut(slot)
                    .ok_or(VmError::InvalidBytecode)? = value;
            }
        }
        let previous = self.program.replace(closure).ok_or(VmError::Finished)?;
        if !tail {
            self.frames.push(ParentFrame {
                closure: previous,
                base: self.base,
                top: self.top,
                ip: self.ip,
                target: i.arg0,
            });
        }
        let old_top = self.top;
        self.base = base;
        self.top = top;
        self.ip = 0;
        if old_top >= top {
            for index in (top..=old_top).rev() {
                *self.native_slot(index)? = Value::Null;
            }
        }
        Ok(())
    }
    pub(super) fn frame_return(&mut self, i: Instruction) -> Result<Option<Value>, VmError> {
        if self.frames.is_empty() {
            return self.return_value(i).map(Some);
        }
        let value = if i.arg0 == 255 {
            Value::Null
        } else {
            self.register(i.arg1)?
        };
        let old_base = self.base;
        let old_top = self.top;
        let parent = self.frames.pop().ok_or(VmError::InvalidBytecode)?;
        self.program = Some(parent.closure);
        self.base = parent.base;
        self.top = parent.top;
        self.ip = parent.ip;
        self.write(parent.target, value)?;
        for index in (old_base.saturating_add(1)..=old_top).rev() {
            *self.native_slot(index)? = Value::Null;
        }
        Ok(None)
    }
    fn grow_stack(&mut self, top: usize) -> Result<(), VmError> {
        // The spare slot models native TOP writes, while every grown slot remains a root.
        let required = top.checked_add(1).ok_or(VmError::Allocation)?;
        if required > self.registers.len() {
            let previous_end = self.registers.len();
            self.registers
                .try_reserve(required.saturating_sub(self.registers.len()))
                .map_err(|_| VmError::Allocation)?;
            self.registers.resize(required, Value::Null);
            *self
                .registers
                .get_mut(previous_end)
                .ok_or(VmError::InvalidBytecode)? =
                std::mem::replace(&mut self.native_scratch, Value::Null);
        }
        Ok(())
    }
}
