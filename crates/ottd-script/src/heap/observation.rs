//! Test-only scalar copies from actual storage; no graph owners or observer actions.
use super::{Handle, Heap, Object, ObjectId, Stored};
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
    Context,
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
                    kind: entry.object.as_deref().map(|object| match object {
                        Object::Table(_) => ObjectKind::Table,
                        Object::Array(_) => ObjectKind::Array,
                        Object::NativeClosure(_) => ObjectKind::NativeClosure,
                        Object::ScriptClosure(_) => ObjectKind::ScriptClosure,
                        Object::Context(_) => ObjectKind::Context,
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
    /// Ordered retirement identities from the reserved test-only release
    /// trace; the trace must have been installed with sufficient capacity.
    pub(crate) fn retired_identities(&self) -> Vec<Identity> {
        self.retired
            .borrow()
            .iter()
            .flatten()
            .copied()
            .map(Identity::from)
            .collect()
    }
    /// Install the test-only retirement trace with pre-reserved capacity.
    pub(crate) fn install_retired_trace(&self, capacity: usize) {
        *self.retired.borrow_mut() = Some(Vec::with_capacity(capacity));
    }
    /// Live reference count of the entry at one identity; zero when absent.
    pub(crate) fn entry_references(&self, identity: Identity) -> usize {
        self.entries
            .borrow()
            .get(identity.index)
            .filter(|entry| entry.generation == identity.generation)
            .and_then(|entry| entry.life.as_ref())
            .map_or(0, |life| life.references.get())
    }
    /// Physical own-slot count of the table at one identity, if it is one.
    pub(crate) fn table_extent(&self, identity: Identity) -> Option<usize> {
        let entries = self.entries.borrow();
        entries
            .get(identity.index)?
            .object
            .as_deref()
            .map(|object| match object {
                Object::Table(data) => data.nodes.len(),
                Object::Array(_)
                | Object::NativeClosure(_)
                | Object::ScriptClosure(_)
                | Object::Context(_) => 0,
            })
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
    /// Snapshot one stored persistent field piece without adding an owner;
    /// its kind and numeric identity are read like a borrowed native value.
    #[expect(
        clippy::unreachable,
        reason = "A materialized context is never a persistent script value"
    )]
    pub(crate) fn stored(stored: &Stored) -> Self {
        match stored {
            Stored::Table(_) => Self::stored_edge(ValueKind::Table, stored),
            Stored::Array(_) => Self::stored_edge(ValueKind::Array, stored),
            Stored::NativeClosure(_) => Self::stored_edge(ValueKind::NativeClosure, stored),
            Stored::ScriptClosure(_) => Self::stored_edge(ValueKind::ScriptClosure, stored),
            Stored::Context(_) => {
                unreachable!("a materialized context is never a persistent script value")
            }
            Stored::Scalar(crate::table::scalar::Scalar::Null) => Self {
                kind: ValueKind::Null,
                identity: None,
            },
            Stored::Scalar(crate::table::scalar::Scalar::Bool(_)) => Self {
                kind: ValueKind::Bool,
                identity: None,
            },
            Stored::Scalar(crate::table::scalar::Scalar::Integer(_)) => Self {
                kind: ValueKind::Integer,
                identity: None,
            },
            Stored::Scalar(crate::table::scalar::Scalar::Float(_)) => Self {
                kind: ValueKind::Float,
                identity: None,
            },
            Stored::Scalar(crate::table::scalar::Scalar::String(_)) => Self {
                kind: ValueKind::String,
                identity: None,
            },
        }
    }
    fn stored_edge(kind: ValueKind, stored: &Stored) -> Self {
        Self {
            kind,
            identity: stored.edge().map(|edge| edge.identity().into()),
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
