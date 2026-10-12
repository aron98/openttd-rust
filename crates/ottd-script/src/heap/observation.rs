//! Test-only scalar copies from actual storage; no graph owners or observer actions.
use super::{Handle, Heap, Object, ObjectId};
use crate::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) index: usize,
    pub(crate) generation: u64,
}
impl From<ObjectId> for Identity {
    fn from(id: ObjectId) -> Self {
        Self {
            index: id.index,
            generation: id.generation,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObjectKind {
    Table,
    Array,
    NativeClosure,
    ScriptClosure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Counts {
    pub(crate) references: usize,
    pub(crate) external: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EntrySnapshot {
    pub(crate) identity: Identity,
    pub(crate) kind: Option<ObjectKind>,
    pub(crate) life: Option<Counts>,
    pub(crate) reference_live_off_chain: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HeapSnapshot {
    pub(crate) entries: Vec<EntrySnapshot>,
    pub(crate) chain: Vec<Identity>,
}
impl Heap {
    pub(crate) fn snapshot(&self) -> HeapSnapshot {
        let entries = self.entries.borrow();
        let chain = self.chain.borrow();
        let copied = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let id = ObjectId {
                    index,
                    generation: entry.generation,
                };
                let life = entry.life.as_ref().map(|life| Counts {
                    references: life.references.get(),
                    external: life.external.get(),
                });
                EntrySnapshot {
                    identity: id.into(),
                    kind: entry.object.as_ref().map(|object| match object {
                        Object::Table(_) => ObjectKind::Table,
                        Object::Array(_) => ObjectKind::Array,
                        Object::NativeClosure(_) => ObjectKind::NativeClosure,
                        Object::ScriptClosure(_) => ObjectKind::ScriptClosure,
                    }),
                    life,
                    reference_live_off_chain: life.is_some_and(|counts| counts.references > 0)
                        && !chain.contains(&id),
                }
            })
            .collect();
        HeapSnapshot {
            entries: copied,
            chain: chain.iter().copied().map(Identity::from).collect(),
        }
    }
}
impl Handle {
    pub(crate) fn observation_identity(&self) -> Identity {
        self.edge.life.id.into()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ValueKind {
    Null,
    Integer,
    Float,
    Bool,
    String,
    Table,
    Array,
    NativeClosure,
    ScriptClosure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ValueSnapshot {
    pub(crate) kind: ValueKind,
    pub(crate) identity: Option<Identity>,
}
impl ValueSnapshot {
    pub(crate) fn handle(kind: ValueKind, handle: &Handle) -> Self {
        Self {
            kind,
            identity: Some(handle.observation_identity()),
        }
    }
    pub(crate) fn value(value: &Value) -> Self {
        let kind = match value {
            Value::Table(table) => return Self::handle(ValueKind::Table, &table.0),
            Value::Array(array) => return Self::handle(ValueKind::Array, &array.0),
            Value::NativeClosure(closure) => {
                return Self::handle(ValueKind::NativeClosure, &closure.0);
            }
            Value::ScriptClosure(closure) => {
                return Self::handle(ValueKind::ScriptClosure, &closure.0);
            }
            Value::Null => ValueKind::Null,
            Value::Integer(_) => ValueKind::Integer,
            Value::Float(_) => ValueKind::Float,
            Value::Bool(_) => ValueKind::Bool,
            Value::String(_) => ValueKind::String,
        };
        Self {
            kind,
            identity: None,
        }
    }
}
