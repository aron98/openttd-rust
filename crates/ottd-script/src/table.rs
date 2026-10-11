//! Native physical tables with scalar keys and realm-owned graph values.
mod data;
mod delegates;
mod hash;
pub(crate) use data::Data;
pub(super) mod nodes;
pub(super) mod scalar;
use crate::{
    Realm, Value, VmError,
    heap::{Handle, Object, Stored},
};
use nodes::Nodes;
use scalar::Scalar;
/// Shared mutable table identity. Aggregate keys are not yet admitted.
#[derive(Clone, Debug)]
pub struct Table(pub(crate) Handle);
impl Table {
    pub(crate) fn reserved(realm: &Realm, capacity: usize) -> Result<Self, VmError> {
        realm
            .heap
            .allocate(realm, Object::Table(Data::new(capacity)?))
            .map(Self)
    }
    #[expect(
        clippy::unreachable,
        reason = "Table handles are constructed only for table entries"
    )]
    fn read<T>(&self, read: impl FnOnce(&Nodes) -> T) -> T {
        self.0.read(|object| match object {
            Object::Table(data) => read(&data.nodes),
            Object::Array(_) | Object::NativeClosure(_) | Object::ScriptClosure(_) => {
                unreachable!()
            }
        })
    }
    #[expect(
        clippy::unreachable,
        reason = "Table handles are constructed only for table entries"
    )]
    fn mutate<T>(&self, mutate: impl FnOnce(&mut Nodes) -> T) -> T {
        self.0.mutate(|object| match object {
            Object::Table(data) => mutate(&mut data.nodes),
            Object::Array(_) | Object::NativeClosure(_) | Object::ScriptClosure(_) => {
                unreachable!()
            }
        })
    }
    pub(crate) fn same_realm(&self, realm: &Realm) -> bool {
        self.0.realm.same(realm)
    }
    /// Whether both handles retain the same object identity.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    /// Number of own slots.
    pub fn len(&self) -> usize {
        self.read(Nodes::len)
    }
    /// Whether this table has no own slots.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Read an own slot without delegation or root fallback.
    /// # Errors
    /// Rejects aggregate keys and unrepresentable float hashing conversions.
    pub fn raw_get(&self, key: Value) -> Result<Option<Value>, VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        if key == Scalar::Null {
            return Ok(None);
        }
        let hash = key.hash()?;
        Ok(self
            .read(|nodes| nodes.get(&key, hash))
            .map(|value| value.value(&self.0.realm)))
    }
    /// Insert or replace an own slot, retaining same-realm aggregate identity.
    /// # Errors
    /// Rejects null keys before values, aggregate keys, invalid hashes and foreign values.
    pub fn new_slot(&self, key: Value, value: Value) -> Result<(), VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        let hash = key.hash()?;
        let value = Stored::from_value(value, &self.0.realm)?;
        self.mutate(|nodes| nodes.insert(key, hash, value))
    }
    /// Replace an existing own slot.
    /// # Errors
    /// Missing slots fail before validating the replacement's realm.
    pub fn set_existing(&self, key: Value, value: Value) -> Result<(), VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        let hash = if key == Scalar::Null { 0 } else { key.hash()? };
        if self.read(|nodes| nodes.get(&key, hash)).is_none() {
            return Err(VmError::MissingIndex);
        }
        let value = Stored::from_value(value, &self.0.realm)?;
        self.mutate(|nodes| nodes.set(&key, hash, value))
    }
    /// Remove an own slot, independently retaining the removed value.
    /// # Errors
    /// Null/missing keys return `MissingIndex`; aggregate keys remain unsupported.
    pub fn delete_slot(&self, key: Value) -> Result<Value, VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        if key == Scalar::Null {
            return Err(VmError::MissingIndex);
        }
        let hash = key.hash()?;
        self.mutate(|nodes| nodes.remove(&key, hash))
            .map(|value| value.value(&self.0.realm))
    }
    /// Allocate an independent table by native physical-order reinsertion.
    /// # Errors
    /// Returns `Allocation` if the node or object storage cannot be reserved.
    pub fn shallow_clone(&self) -> Result<Self, VmError> {
        let nodes = self.read(Nodes::duplicate)?;
        let copy = self
            .0
            .realm
            .heap
            .allocate(
                &self.0.realm,
                Object::Table(Data {
                    delegate: None,
                    nodes,
                }),
            )
            .map(Self)?;
        self.0.realm.heap.copy_delegate(&self.0, &copy.0)?;
        Ok(copy)
    }
    pub(crate) fn next(&self, cursor: usize) -> Option<(usize, Value, Value)> {
        self.read(|nodes| nodes.next(cursor))
            .map(|(next, key, value)| (next, key.value(), value.value(&self.0.realm)))
    }
    pub(crate) fn lookup(&self, key: Value) -> Result<Value, VmError> {
        if let Some(value) = self.raw_get(key.clone())? {
            return Ok(value);
        }
        if let Value::String(key) = key {
            if matches!(
                key.as_bytes(),
                b"len"
                    | b"rawget"
                    | b"rawset"
                    | b"rawdelete"
                    | b"rawin"
                    | b"weakref"
                    | b"tostring"
                    | b"clear"
            ) {
                return Err(VmError::UnsupportedRuntimeValue);
            }
        }
        Err(VmError::MissingIndex)
    }
    #[cfg(test)]
    pub(crate) fn watch(&self) -> impl Fn() -> Option<Self> + use<> {
        let watch = self.0.watch();
        move || watch().map(Self)
    }
}
impl PartialEq for Table {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}
impl Eq for Table {}
impl Realm {
    /// Allocate an independent empty table in this realm.
    /// # Errors
    /// Returns `Allocation` if node or object storage cannot be reserved.
    pub fn table(&self) -> Result<Table, VmError> {
        Table::reserved(self, 0)
    }
}
#[cfg(test)]
mod tests;
