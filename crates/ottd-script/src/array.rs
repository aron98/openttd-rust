//! Array identity with structurally scalar storage; no container edges or cycles.
use crate::{ByteString, Realm, Value, VmError};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
enum Element {
    Null,
    Bool(bool),
    Integer(i64),
    Float(u32),
    String(ByteString),
}
impl Element {
    fn from_value(value: Value, realm: &Realm) -> Result<Self, VmError> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(value)),
            Value::Integer(value) => Ok(Self::Integer(value)),
            Value::Float(value) => Ok(Self::Float(value)),
            Value::String(value) => Ok(Self::String(realm.string(value.as_bytes()))),
            Value::Array(_) => Err(VmError::UnsupportedArrayElement),
        }
    }
    fn value(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(*value),
            Self::Integer(value) => Value::Integer(*value),
            Self::Float(value) => Value::Float(*value),
            Self::String(value) => Value::String(value.clone()),
        }
    }
}
#[derive(Debug)]
struct Data {
    realm: Realm,
    elements: RefCell<Vec<Element>>,
}
/// Shared mutable array identity containing only scalar elements.
/// Clone retains the allocation; equality compares identity, not contents.
#[derive(Clone, Debug)]
pub struct Array(Rc<Data>);
impl Array {
    pub(crate) fn reserved(realm: &Realm, capacity: usize) -> Result<Self, VmError> {
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(capacity)
            .map_err(|_| VmError::Allocation)?;
        Ok(Self(Rc::new(Data {
            realm: realm.clone(),
            elements: RefCell::new(elements),
        })))
    }
    pub(crate) fn append(&self, value: Value) -> Result<(), VmError> {
        let value = Element::from_value(value, &self.0.realm)?;
        let mut elements = self.0.elements.borrow_mut();
        elements.try_reserve(1).map_err(|_| VmError::Allocation)?;
        elements.push(value);
        Ok(())
    }
    pub(crate) fn same_realm(&self, realm: &Realm) -> bool {
        self.0.realm.same(realm)
    }
    /// Number of currently initialized elements.
    pub fn len(&self) -> usize {
        self.0.elements.borrow().len()
    }
    /// Whether this array has no initialized elements.
    pub fn is_empty(&self) -> bool {
        self.0.elements.borrow().is_empty()
    }
    /// Read a scalar element, retaining a returned string independently.
    pub fn get(&self, index: i64) -> Option<Value> {
        self.0
            .elements
            .borrow()
            .get(usize::try_from(index).ok()?)
            .map(Element::value)
    }
    /// Replace an existing scalar element; strings are interned in this array's realm.
    ///
    /// # Errors
    /// Returns `MissingIndex` for an absent index before checking the value.
    /// Returns `UnsupportedArrayElement` for an array value, without replacing the element.
    pub fn set(&self, index: i64, value: Value) -> Result<(), VmError> {
        let index = usize::try_from(index).map_err(|_| VmError::MissingIndex)?;
        let mut elements = self.0.elements.borrow_mut();
        let target = elements.get_mut(index).ok_or(VmError::MissingIndex)?;
        *target = Element::from_value(value, &self.0.realm)?;
        Ok(())
    }
    /// Whether both handles refer to the same mutable allocation.
    pub fn same_identity(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    #[cfg(test)]
    pub(crate) fn watch(&self) -> impl Fn() -> Option<Self> + use<> {
        let weak = Rc::downgrade(&self.0);
        move || weak.upgrade().map(Self)
    }
}
impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        self.same_identity(other)
    }
}
impl Eq for Array {}
impl Realm {
    /// Allocate an independent array with scalar elements in this realm.
    ///
    /// # Errors
    /// Returns `UnsupportedArrayElement` for aggregate elements or `Allocation`
    /// if element storage cannot be reserved. No partial array is published.
    pub fn array(&self, values: Vec<Value>) -> Result<Array, VmError> {
        let array = Array::reserved(self, values.len())?;
        for value in values {
            array.append(value)?;
        }
        Ok(array)
    }
}
