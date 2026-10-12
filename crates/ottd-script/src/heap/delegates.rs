//! Delegate installation retains the new edge, releases the old edge, then publishes.
use super::ownership::Ownership;
use super::{Edge, Handle, Heap, Object, ObjectId};
use crate::VmError;

impl Heap {
    pub(crate) fn attach_delegate(
        &self,
        owner: &Handle,
        delegate: Option<&Handle>,
    ) -> Result<(), VmError> {
        self.attach_delegate_id(owner, delegate.map(|handle| handle.edge.life.id))
    }
    pub(crate) fn copy_delegate(&self, source: &Handle, copy: &Handle) -> Result<(), VmError> {
        let delegate = source.read(|object| match object {
            Object::Table(data) => Ok(data.delegate.as_ref().map(|edge| edge.life.id)),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Context(_) => Err(VmError::InvalidBytecode),
        })?;
        self.attach_delegate_id(copy, delegate)
    }
    pub(super) fn attach_delegate_id(
        &self,
        owner: &Handle,
        delegate: Option<ObjectId>,
    ) -> Result<(), VmError> {
        let replacement = {
            let entries = self.entries.borrow();
            let mut current = delegate;
            while let Some(id) = current {
                if id == owner.edge.life.id {
                    return Err(VmError::DelegateCycle);
                }
                let entry = entries
                    .get(id.index)
                    .filter(|entry| entry.generation == id.generation)
                    .ok_or(VmError::InvalidBytecode)?;
                let Some(Object::Table(data)) = entry.object.as_deref() else {
                    return Err(VmError::InvalidBytecode);
                };
                current = data.delegate.as_ref().map(|edge| edge.life.id);
            }
            delegate
                .map(|id| {
                    let life = entries
                        .get(id.index)
                        .and_then(|entry| entry.life.as_ref())
                        .ok_or(VmError::InvalidBytecode)?;
                    Ok(Edge::retain(life, Ownership::Stored))
                })
                .transpose()?
        };
        let old = owner.mutate(|object| match object {
            Object::Table(data) => Ok(data.delegate.take()),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Context(_) => Err(VmError::InvalidBytecode),
        })?;
        drop(old);
        self.release_pending();
        owner.mutate(|object| match object {
            Object::Table(data) => {
                data.delegate = replacement;
                Ok(())
            }
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Context(_) => Err(VmError::InvalidBytecode),
        })
    }
}
