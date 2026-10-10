//! Compiler bindings own interned names and values independently of programs.
#![expect(
    clippy::redundant_pub_crate,
    reason = "Bindings cross private compiler and realm modules"
)]
use super::{ByteString, Realm};
use crate::Value;
use std::{collections::BTreeMap, rc::Rc};

#[derive(Clone, Debug)]
pub(crate) enum Scalar {
    Integer(i64),
    Float(u32),
    String(ByteString),
}
impl Scalar {
    pub(crate) fn value(&self) -> Value {
        match self {
            Self::Integer(value) => Value::Integer(*value),
            Self::Float(bits) => Value::Float(*bits),
            Self::String(value) => Value::String(value.clone()),
        }
    }
}
pub(crate) type Members = BTreeMap<Vec<u8>, (ByteString, Scalar)>;
#[derive(Clone, Debug)]
pub(crate) enum Binding {
    Scalar(Scalar),
    Enum(Rc<Members>),
}
pub(super) type Table = BTreeMap<Vec<u8>, (ByteString, Binding)>;
impl Realm {
    pub(crate) fn constant(&self, name: &str) -> Option<Binding> {
        self.constant_bytes(name.as_bytes())
    }
    pub(crate) fn constant_bytes(&self, name: &[u8]) -> Option<Binding> {
        self.constants
            .borrow()
            .get(name)
            .map(|(_, value)| value.clone())
    }
    pub(crate) fn publish(&self, name: ByteString, value: Binding) {
        // StringData::drop borrows the separate pool, never this table.
        self.constants
            .borrow_mut()
            .insert(name.as_bytes().to_vec(), (name, value));
    }
}

#[cfg(test)]
mod corpus;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod sessions;
