//! Array identity and elements use the same realm graph as tables.
use crate::{
    Realm, Value, VmError,
    heap::{Handle, Object, Stored},
};
/// Shared mutable array identity; internal aggregate edges do not own the realm.
#[derive(Clone, Debug)]
pub struct Array(pub(crate) Handle);
impl Array {
    pub(crate) fn reserved(realm: &Realm, capacity: usize) -> Result<Self, VmError> {
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(capacity)
            .map_err(|_| VmError::Allocation)?;
        realm
            .heap
            .allocate(realm, Object::Array(elements))
            .map(Self)
    }
    #[expect(
        clippy::unreachable,
        reason = "Array handles are constructed only for array entries"
    )]
    fn read<T>(&self, read: impl FnOnce(&[Stored]) -> T) -> T {
        self.0.read(|object| match object {
            Object::Array(values) => read(values),
            Object::Table(_) | Object::NativeClosure(_) | Object::ScriptClosure(_) => {
                unreachable!()
            }
        })
    }
    #[expect(
        clippy::unreachable,
        reason = "Array handles are constructed only for array entries"
    )]
    fn mutate<T>(&self, mutate: impl FnOnce(&mut Vec<Stored>) -> T) -> T {
        self.0.mutate(|object| match object {
            Object::Array(values) => mutate(values),
            Object::Table(_) | Object::NativeClosure(_) | Object::ScriptClosure(_) => {
                unreachable!()
            }
        })
    }
    pub(crate) fn append(&self, value: Value) -> Result<(), VmError> {
        let value = Stored::from_value(value, &self.0.realm)?;
        self.mutate(|values| {
            values.try_reserve(1).map_err(|_| VmError::Allocation)?;
            values.push(value);
            Ok(())
        })
    }
    /// Allocate an independent array retaining the same element identities.
    /// # Errors
    /// Returns `Allocation` if element or object storage cannot be reserved.
    pub fn shallow_clone(&self) -> Result<Self, VmError> {
        let elements = self.read(|source| {
            let mut values = Vec::new();
            values
                .try_reserve_exact(source.len())
                .map_err(|_| VmError::Allocation)?;
            values.extend_from_slice(source);
            Ok(values)
        })?;
        self.0
            .realm
            .heap
            .allocate(&self.0.realm, Object::Array(elements))
            .map(Self)
    }
    pub(crate) fn same_realm(&self, realm: &Realm) -> bool {
        self.0.realm.same(realm)
    }
    /// Number of initialized elements.
    pub fn len(&self) -> usize {
        self.read(<[Stored]>::len)
    }
    /// Whether this array has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Read an element, retaining returned strings and aggregates independently.
    pub fn get(&self, index: i64) -> Option<Value> {
        let index = usize::try_from(index).ok()?;
        self.read(|values| values.get(index).cloned())
            .map(|value| value.value(&self.0.realm))
    }
    /// Replace an existing element, retaining same-realm aggregate identity.
    /// # Errors
    /// Missing indices fail before foreign values are rejected with `RealmMismatch`.
    pub fn set(&self, index: i64, value: Value) -> Result<(), VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::MissingIndex)?;
        if index >= self.len() {
            return Err(VmError::MissingIndex);
        }
        let value = Stored::from_value(value, &self.0.realm)?;
        self.mutate(|values| {
            *values.get_mut(index).ok_or(VmError::MissingIndex)? = value;
            Ok(())
        })
    }
    /// Whether both handles retain the same mutable object.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    #[cfg(test)]
    pub(crate) fn watch(&self) -> impl Fn() -> Option<Self> + use<> {
        let watch = self.0.watch();
        move || watch().map(Self)
    }
}
impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}
impl Eq for Array {}
impl Realm {
    /// Allocate an independent array, retaining same-realm aggregate elements.
    /// # Errors
    /// Foreign aggregates return `RealmMismatch`; allocation can return `Allocation`.
    /// No partial array is published on failure.
    pub fn array(&self, values: Vec<Value>) -> Result<Array, VmError> {
        let array = Array::reserved(self, values.len())?;
        for value in values {
            array.append(value)?;
        }
        Ok(array)
    }
}
