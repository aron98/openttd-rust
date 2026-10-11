//! Delegate access retains the existing table identity and checks cycles before mutation.
use super::Table;
use crate::{VmError, heap::Handle};
impl Table {
    /// Retain this table's immediate delegate, if attached.
    pub fn delegate(&self) -> Option<Self> {
        self.0.read(|object| match object {
            crate::heap::Object::Table(data) => data
                .delegate
                .as_ref()
                .map(|edge| Self(Handle::from_edge(edge, &self.0.realm))),
            crate::heap::Object::Array(_)
            | crate::heap::Object::NativeClosure(_)
            | crate::heap::Object::ScriptClosure(_) => None,
        })
    }
    /// Attach or detach a delegate without changing this table's own slots.
    /// # Errors
    /// Rejects a foreign realm or a direct/transitive delegate cycle before mutation.
    pub fn set_delegate(&self, delegate: Option<&Self>) -> Result<(), VmError> {
        if delegate.is_some_and(|table| !table.same_realm(&self.0.realm)) {
            return Err(VmError::RealmMismatch);
        }
        self.0
            .realm
            .heap
            .attach_delegate(&self.0, delegate.map(|table| &table.0))
    }
}
