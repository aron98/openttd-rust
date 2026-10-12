//! Atomic native bodies share the main budget loop and original caller cleanup.
use super::{Instruction, Value, Vm, VmError};
use crate::{Array, NativeClosure, NativeFunction, heap::Operand};
/// The admitted native frame owns its `CallInfo` closure independently of CALL's
/// retained local callee and the VM temporary. Its arguments remain live slots.
struct NativeFrame {
    closure: NativeClosure,
    base: usize,
    top: usize,
    count: u8,
}
impl Vm<'_> {
    pub(super) fn prepare_call(&mut self, i: Instruction) -> Result<(), VmError> {
        let receiver = self.register(i32::from(i.arg2))?;
        let get = if i.opcode == 0x08 {
            Instruction { opcode: 0x09, ..i }
        } else {
            Instruction {
                opcode: 0x0e,
                arg1: i32::from(i.arg2),
                arg2: u8::try_from(i.arg1).map_err(|_| VmError::InvalidBytecode)?,
                ..i
            }
        };
        let closure = self.root_get(get)?;
        self.write(i.arg3, receiver)?;
        self.write(i.arg0, closure)
    }
    pub(super) fn native_failure(&mut self, message: &[u8], error: VmError) -> VmError {
        self.raised = true;
        let value = Value::String(self.runner.get().root.as_ref().realm.string(message));
        self.runner.get_mut().native_error.set(value);
        error
    }
    pub(super) fn call(
        &mut self,
        i: Instruction,
        closure: NativeClosure,
    ) -> Result<Value, VmError> {
        match closure.function() {
            NativeFunction::Default(function) => {
                return Err(VmError::UnsupportedNativeFunction(function));
            }
            NativeFunction::Assert
            | NativeFunction::Print
            | NativeFunction::Array
            | NativeFunction::Type => {}
        }
        let count = i64::from(i.arg3);
        let check = closure.parameter_check();
        if (check > 0 && count != check)
            || (check < 0 && count < check.checked_neg().ok_or(VmError::InvalidBytecode)?)
        {
            return Err(self.native_failure(b"wrong number of parameters", VmError::NativeCall));
        }
        let base = self
            .base
            .checked_add(usize::from(i.arg2))
            .ok_or(VmError::InvalidBytecode)?;
        let top = base
            .checked_add(usize::from(i.arg3))
            .ok_or(VmError::InvalidBytecode)?;
        if i.arg3 == 0 || top > self.top {
            return Err(VmError::InvalidBytecode);
        }
        let first = self.register(
            i32::from(i.arg2)
                .checked_add(1)
                .ok_or(VmError::InvalidBytecode)?,
        )?;
        if closure.function() == NativeFunction::Array
            && !matches!(first, Value::Integer(_) | Value::Float(_))
        {
            let Value::String(kind) = first.type_name(&self.runner.get().root.as_ref().realm)
            else {
                return Err(VmError::InvalidBytecode);
            };
            let mut message = b"parameter 1 has an invalid type '".to_vec();
            message.extend_from_slice(kind.as_bytes());
            message.extend_from_slice(b"' ; expected: 'integer|float'");
            return Err(self.native_failure(&message, VmError::NativeCall));
        }
        let first = Operand::new(first)?;
        let frame = NativeFrame {
            closure: closure.clone(),
            base,
            top,
            count: i.arg3,
        };
        let result = self.native_body(&frame, first.value());
        // Successful CallNative replaces CALL's local callee with the result
        // before popping CallInfo; failure leaves that local owner until unwind.
        let failure_owner = result.is_err().then_some(closure);
        drop(frame);
        drop(first);
        // CallNative restores the old base before adding its already absolute
        // argument base. Preserve this pinned arithmetic, including the top slot.
        let threshold = self
            .origin
            .checked_add(self.base)
            .and_then(|origin| origin.checked_add(base))
            .ok_or(VmError::InvalidBytecode)?;
        for index in (0..=self.top).rev().take_while(|index| *index > threshold) {
            *self.native_slot(index)? = Value::Null;
        }
        drop(failure_owner);
        result
    }
    pub(super) fn native_slot(&mut self, index: usize) -> Result<&mut Value, VmError> {
        if index == self.registers.len() {
            return Ok(&mut self.native_scratch);
        }
        self.registers
            .get_mut(index)
            .ok_or(VmError::InvalidBytecode)
    }
    fn native_body(&mut self, frame: &NativeFrame, first: &Value) -> Result<Value, VmError> {
        let NativeFrame {
            closure,
            base,
            top,
            count,
        } = frame;
        let (base, top, count) = (*base, *top, *count);
        match closure.function() {
            NativeFunction::Default(function) => Err(VmError::UnsupportedNativeFunction(function)),
            NativeFunction::Assert => {
                if first.is_false() {
                    return Err(self.native_failure(b"assertion failed", VmError::NativeCall));
                }
                Ok(Value::Null)
            }
            NativeFunction::Print => {
                if let Value::Table(table) = first {
                    table.refuse_metamethod(b"_tostring")?;
                }
                let bytes = first.text()?;
                let string = self.runner.get().root.as_ref().realm.string(&bytes);
                *self.native_slot(top)? = Value::String(string);
                if let Some(sink) = &mut self.runner.get_mut().print_sink {
                    sink.print(&bytes);
                }
                Ok(Value::Null)
            }
            NativeFunction::Type => {
                *self.native_slot(top)? = first.type_name(&self.runner.get().root.as_ref().realm);
                Ok(std::mem::replace(self.native_slot(top)?, Value::Null))
            }
            NativeFunction::Array => {
                let size = match first {
                    Value::Integer(size) => *size,
                    Value::Float(bits) => super::arrays::float_index(*bits)?,
                    Value::Null
                    | Value::Bool(_)
                    | Value::String(_)
                    | Value::Array(_)
                    | Value::Table(_)
                    | Value::NativeClosure(_)
                    | Value::ScriptClosure(_) => return Err(VmError::InvalidBytecode),
                };
                let error = if size < 0 {
                    Some(self.native_failure(
                        format!("can't create/resize array with/to size {size}").as_bytes(),
                        VmError::NativeCall,
                    ))
                } else {
                    None
                };
                let length = usize::try_from(size.max(0)).map_err(|_| VmError::Allocation)?;
                let array = Array::reserved(&self.runner.get().root.as_ref().realm, length)?;
                let fill = if count > 2 {
                    self.registers
                        .get(base.checked_add(2).ok_or(VmError::InvalidBytecode)?)
                        .cloned()
                        .ok_or(VmError::InvalidBytecode)?
                } else {
                    Value::Null
                };
                for _ in 0..length {
                    array.append(fill.clone())?;
                }
                *self.native_slot(top)? = Value::Array(array);
                if let Some(error) = error {
                    return Err(error);
                }
                Ok(std::mem::replace(self.native_slot(top)?, Value::Null))
            }
        }
    }
}
