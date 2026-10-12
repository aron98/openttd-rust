//! Closure construction publishes only after captures and defaults succeed.
use super::{Instruction, Value, Vm, VmError};
use crate::{
    ScriptClosure,
    heap::{Object, Stored},
    prototype::Capture,
};
use std::rc::Rc;
impl Vm<'_> {
    pub(super) fn load_outer(&self, index: i32) -> Result<Value, VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::InvalidBytecode)?;
        let closure = self.program.as_ref().ok_or(VmError::Finished)?;
        closure
            .read(|data| {
                data.outers
                    .get(index)
                    .map(|value| value.value(&closure.0.realm))
            })
            .ok_or(VmError::InvalidBytecode)
    }
    #[expect(
        clippy::unreachable,
        reason = "The construction token was allocated with script closure storage"
    )]
    pub(super) fn create_closure(&mut self, i: Instruction) -> Result<Value, VmError> {
        if i.arg2 != 0 || i.arg3 != 0 {
            return Err(VmError::InvalidBytecode);
        }
        let index = usize::try_from(i.arg1).map_err(|_| VmError::InvalidBytecode)?;
        let proto = self
            .program()?
            .0
            .children
            .get(index)
            .cloned()
            .ok_or(VmError::InvalidBytecode)?;
        let realm = self.runner.get().root.as_ref().realm.clone();
        let unpublished = realm.heap.unpublished(
            &realm,
            Object::ScriptClosure(crate::script_closure::Data {
                prototype: Rc::clone(&proto),
                outers: Vec::new(),
                defaults: Vec::new(),
            }),
        )?;
        for capture in &proto.captures {
            let value = match capture {
                Capture::Local(index) => self.register(i32::from(*index))?,
                Capture::Outer(index) => self.load_outer(i32::from(*index))?,
                Capture::Symbol(key) => {
                    unpublished.mutate(|object| match object {
                        Object::ScriptClosure(data) => data.outers.push(Stored::null()),
                        Object::Array(_) | Object::Table(_) | Object::NativeClosure(_) => {
                            unreachable!()
                        }
                    });
                    match self.lookup_value(&self.register(0)?, Value::String(key.clone())) {
                        Ok(value) => value,
                        Err(error @ (VmError::MissingIndex | VmError::UnsupportedReceiver)) => {
                            return Err(self.index_failure(&Value::String(key.clone()), error));
                        }
                        Err(error) => return Err(error),
                    }
                }
            };
            let value = Stored::from_value(value, &realm)?;
            unpublished.mutate(|object| match object {
                Object::ScriptClosure(data) => {
                    if matches!(capture, Capture::Symbol(_)) {
                        let _null = data.outers.pop();
                    }
                    data.outers.push(value);
                }
                Object::Array(_) | Object::Table(_) | Object::NativeClosure(_) => unreachable!(),
            });
        }
        for index in &proto.defaults {
            let value = Stored::from_value(self.register(i32::from(*index))?, &realm)?;
            unpublished.mutate(|object| match object {
                Object::ScriptClosure(data) => data.defaults.push(value),
                Object::Array(_) | Object::Table(_) | Object::NativeClosure(_) => unreachable!(),
            });
        }
        Ok(Value::ScriptClosure(ScriptClosure(unpublished.publish())))
    }
}
