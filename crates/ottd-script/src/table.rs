//! Native physical table layout with scalar slots and independently owned identity.
mod hash;
mod nodes;
mod scalar;
use crate::{Realm, Value, VmError};
use nodes::Nodes;
use scalar::Scalar;
use std::{cell::RefCell, rc::Rc};
#[derive(Debug)]
struct Data {
    realm: Realm,
    nodes: RefCell<Nodes>,
}
/// Shared mutable native table identity with scalar keys and values.
/// Nested objects and delegates await the realm-owned graph heap.
#[derive(Clone, Debug)]
pub struct Table(Rc<Data>);
impl Table {
    pub(crate) fn reserved(realm: &Realm, capacity: usize) -> Result<Self, VmError> {
        Ok(Self(Rc::new(Data {
            realm: realm.clone(),
            nodes: RefCell::new(Nodes::new(capacity)?),
        })))
    }
    pub(crate) fn same_realm(&self, realm: &Realm) -> bool {
        self.0.realm.same(realm)
    }
    /// Whether both handles retain the same allocation.
    pub fn same_identity(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    /// Number of own slots.
    pub fn len(&self) -> usize {
        self.0.nodes.borrow().len()
    }
    /// Whether this table has no own slots.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Read an own slot, without default delegates or root fallback.
    /// # Errors
    /// Rejects aggregate keys and unrepresentable float hashing conversions.
    pub fn raw_get(&self, key: Value) -> Result<Option<Value>, VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        if key == Scalar::Null {
            return Ok(None);
        }
        Ok(self
            .0
            .nodes
            .borrow()
            .get(&key, key.hash()?)
            .map(|value| value.value()))
    }
    /// Insert or replace an own slot.
    /// # Errors
    /// Rejects null keys before checking values, aggregates and invalid float conversions.
    pub fn new_slot(&self, key: Value, value: Value) -> Result<(), VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        let hash = key.hash()?;
        let value = Scalar::from_value(value, &self.0.realm)?;
        self.0.nodes.borrow_mut().insert(key, hash, value)
    }
    /// Replace an existing own slot.
    /// # Errors
    /// Missing slots fail before checking the replacement value; aggregate keys unsupported.
    pub fn set_existing(&self, key: Value, value: Value) -> Result<(), VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        // SQTable::Set performs raw lookup even for null: an empty node can retain
        // an invisible value until rehash/release, without becoming an own slot.
        let hash = if key == Scalar::Null { 0 } else { key.hash()? };
        let mut nodes = self.0.nodes.borrow_mut();
        if nodes.get(&key, hash).is_none() {
            return Err(VmError::MissingIndex);
        }
        nodes.set(&key, hash, Scalar::from_value(value, &self.0.realm)?)
    }
    /// Remove an own slot and retain the removed scalar value.
    /// # Errors
    /// Null and missing keys return `MissingIndex`; aggregate keys are unsupported.
    pub fn delete_slot(&self, key: Value) -> Result<Value, VmError> {
        let key = Scalar::from_value(key, &self.0.realm)?;
        if key == Scalar::Null {
            return Err(VmError::MissingIndex);
        }
        self.0
            .nodes
            .borrow_mut()
            .remove(&key, key.hash()?)
            .map(|value| value.value())
    }
    /// Allocate an independent table by native physical-order reinsertion.
    /// # Errors
    /// Returns `Allocation` if the copied node storage cannot be reserved.
    pub fn shallow_clone(&self) -> Result<Self, VmError> {
        let nodes = self.0.nodes.borrow().duplicate()?;
        Ok(Self(Rc::new(Data {
            realm: self.0.realm.clone(),
            nodes: RefCell::new(nodes),
        })))
    }
    pub(crate) fn next(&self, cursor: usize) -> Option<(usize, Value, Value)> {
        self.0
            .nodes
            .borrow()
            .next(cursor)
            .map(|(next, key, value)| (next, key.value(), value.value()))
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
        let weak = Rc::downgrade(&self.0);
        move || weak.upgrade().map(Self)
    }
}
impl PartialEq for Table {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}
impl Eq for Table {}
impl Realm {
    /// Allocate an independent empty scalar table.
    /// # Errors
    /// Returns `Allocation` if the initial node array cannot be reserved.
    pub fn table(&self) -> Result<Table, VmError> {
        Table::reserved(self, 0)
    }
}

#[cfg(test)]
mod tests;
