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
            | crate::heap::Object::ScriptClosure(_)
            | crate::heap::Object::Context(_) => None,
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
    pub(crate) fn refuse_metamethod(&self, name: &[u8]) -> Result<(), VmError> {
        if let Some(delegate) = self.delegate() {
            let key = crate::Value::String(self.0.realm.string(name));
            if delegate.raw_get(key)?.is_some() {
                return Err(VmError::UnsupportedMetamethod);
            }
        }
        Ok(())
    }
    pub(crate) fn raw_chain_get(&self, key: crate::Value) -> Result<Option<crate::Value>, VmError> {
        if let Some(value) = self.raw_get(key.clone())? {
            return Ok(Some(value));
        }
        self.delegate()
            .map_or_else(|| Ok(None), |delegate| delegate.raw_chain_get(key))
    }
    pub(crate) fn set_delegated(
        &self,
        key: crate::Value,
        value: crate::Value,
    ) -> Result<(), VmError> {
        match self.set_existing(key.clone(), value.clone()) {
            Ok(()) => return Ok(()),
            Err(VmError::MissingIndex) => {}
            Err(error) => return Err(error),
        }
        if let Some(delegate) = self.delegate() {
            match delegate.set_delegated(key, value) {
                Ok(()) => return Ok(()),
                Err(VmError::MissingIndex) => {}
                Err(error) => return Err(error),
            }
        }
        self.refuse_metamethod(b"_set")?;
        Err(VmError::MissingIndex)
    }
}
