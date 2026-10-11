//! Stored aggregate edges carry object identity without a realm owner.
use super::{Edge, Handle};
use crate::{Array, Realm, Table, Value, VmError, table::scalar::Scalar};
#[derive(Clone, Debug)]
pub(crate) enum Stored {
    NativeClosure(Edge),
    Scalar(Scalar),
    Array(Edge),
    Table(Edge),
}
impl Stored {
    pub(crate) const fn null() -> Self {
        Self::Scalar(Scalar::Null)
    }
    pub(crate) fn from_value(value: Value, realm: &Realm) -> Result<Self, VmError> {
        match value {
            Value::NativeClosure(closure) => {
                if !closure.same_realm(realm) {
                    return Err(VmError::RealmMismatch);
                }
                Ok(Self::NativeClosure(closure.0.edge.internal()))
            }
            Value::Array(array) => {
                if !array.same_realm(realm) {
                    return Err(VmError::RealmMismatch);
                }
                Ok(Self::Array(array.0.edge.internal()))
            }
            Value::Table(table) => {
                if !table.same_realm(realm) {
                    return Err(VmError::RealmMismatch);
                }
                Ok(Self::Table(table.0.edge.internal()))
            }
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_) => Scalar::from_value(value, realm).map(Self::Scalar),
        }
    }
    pub(crate) fn value(&self, realm: &Realm) -> Value {
        match self {
            Self::NativeClosure(edge) => {
                Value::NativeClosure(crate::NativeClosure(Handle::from_edge(edge, realm)))
            }
            Self::Scalar(value) => value.value(),
            Self::Array(edge) => Value::Array(Array(Handle::from_edge(edge, realm))),
            Self::Table(edge) => Value::Table(Table(Handle::from_edge(edge, realm))),
        }
    }
    pub(super) const fn edge(&self) -> Option<&Edge> {
        match self {
            Self::Scalar(_) => None,
            Self::Array(edge) | Self::Table(edge) | Self::NativeClosure(edge) => Some(edge),
        }
    }
}
