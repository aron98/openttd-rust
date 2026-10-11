//! Scalar-only slots make cycles impossible until the realm heap migration.
use crate::{ByteString, Realm, Value, VmError};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Scalar {
    Null,
    Bool(bool),
    Integer(i64),
    Float(u32),
    String(ByteString),
}
impl Scalar {
    pub(super) fn from_value(value: Value, realm: &Realm) -> Result<Self, VmError> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(value)),
            Value::Integer(value) => Ok(Self::Integer(value)),
            Value::Float(value) => Ok(Self::Float(value)),
            Value::String(value) => Ok(Self::String(realm.string(value.as_bytes()))),
            Value::Array(_) | Value::Table(_) => Err(VmError::UnsupportedTableElement),
        }
    }
    pub(super) fn value(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(*value),
            Self::Integer(value) => Value::Integer(*value),
            Self::Float(value) => Value::Float(*value),
            Self::String(value) => Value::String(value.clone()),
        }
    }
    pub(super) fn hash(&self) -> Result<u64, VmError> {
        match self {
            Self::Null => Err(VmError::NullIndex),
            Self::Bool(value) => Ok(u64::from(*value)),
            Self::Integer(value) => Ok(u64::from_ne_bytes(value.to_ne_bytes())),
            Self::Float(bits) => crate::vm::arrays::float_index(*bits)
                .map(|value| u64::from_ne_bytes(value.to_ne_bytes())),
            Self::String(value) => super::hash::string(value.as_bytes()),
        }
    }
}
