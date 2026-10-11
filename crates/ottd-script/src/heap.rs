//! Realm-owned objects. Internal edges retain identities without owning the heap.
mod value;
use crate::{Realm, VmError, table::nodes::Nodes};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
pub(crate) use value::Stored;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ObjectId {
    index: usize,
    generation: u64,
}
#[derive(Debug)]
struct Life {
    id: ObjectId,
    created: u64,
    heap: Weak<Heap>,
    references: Cell<usize>,
    external: Cell<usize>,
}
/// An ownership edge; its metadata has no strong link to object storage.
#[derive(Debug)]
pub(crate) struct Edge {
    life: Rc<Life>,
    external: bool,
}
impl Edge {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "An allocated edge bounds each ownership count below usize::MAX"
    )]
    fn retain(life: &Rc<Life>, external: bool) -> Self {
        life.references.set(life.references.get() + 1);
        if external {
            life.external.set(life.external.get() + 1);
        }
        Self {
            life: Rc::clone(life),
            external,
        }
    }
    pub(crate) fn internal(&self) -> Self {
        Self::retain(&self.life, false)
    }
    fn external(&self) -> Self {
        Self::retain(&self.life, true)
    }
}
impl Clone for Edge {
    fn clone(&self) -> Self {
        Self::retain(&self.life, self.external)
    }
}
impl Drop for Edge {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "Each edge owns one live reference, and external edges one external reference"
    )]
    fn drop(&mut self) {
        let life = &self.life;
        if self.external {
            life.external.set(life.external.get() - 1);
        }
        life.references.set(life.references.get() - 1);
        if let Some(heap) = life.heap.upgrade() {
            if life.references.get() == 0 {
                heap.pending.borrow_mut().push(life.id);
            }
            if self.external {
                heap.release_pending();
            }
        }
    }
}
#[derive(Debug)]
pub(crate) enum Object {
    Table(Nodes),
    Array(Vec<Stored>),
}
impl Object {
    fn edges(&self) -> impl Iterator<Item = &Edge> {
        let (table, array) = match self {
            Self::Table(nodes) => (Some(nodes), None),
            Self::Array(values) => (None, Some(values)),
        };
        table
            .into_iter()
            .flat_map(Nodes::values)
            .chain(array.into_iter().flatten())
            .filter_map(Stored::edge)
    }
}
#[derive(Debug)]
struct Entry {
    generation: u64,
    life: Option<Rc<Life>>,
    object: Option<Object>,
}
#[derive(Debug, Default)]
pub(crate) struct Heap {
    entries: RefCell<Vec<Entry>>,
    pending: RefCell<Vec<ObjectId>>,
    releasing: Cell<bool>,
    next_creation: Cell<u64>,
}
impl Heap {
    pub(crate) fn allocate(
        self: &Rc<Self>,
        realm: &Realm,
        object: Object,
    ) -> Result<Handle, VmError> {
        let created = self
            .next_creation
            .get()
            .checked_add(1)
            .ok_or(VmError::Allocation)?;
        let mut entries = self.entries.borrow_mut();
        let vacant = entries
            .iter()
            .position(|entry| entry.life.is_none() && entry.generation < u64::MAX);
        let index = if let Some(index) = vacant {
            index
        } else {
            entries.try_reserve(1).map_err(|_| VmError::Allocation)?;
            entries.push(Entry {
                generation: 0,
                life: None,
                object: None,
            });
            entries.len().checked_sub(1).ok_or(VmError::Allocation)?
        };
        let entry = entries.get_mut(index).ok_or(VmError::InvalidBytecode)?;
        entry.generation = entry.generation.checked_add(1).ok_or(VmError::Allocation)?;
        let life = Rc::new(Life {
            id: ObjectId {
                index,
                generation: entry.generation,
            },
            created,
            heap: Rc::downgrade(self),
            references: Cell::new(0),
            external: Cell::new(0),
        });
        let edge = Edge::retain(&life, true);
        entry.life = Some(life);
        entry.object = Some(object);
        self.next_creation.set(created);
        Ok(Handle {
            edge,
            realm: realm.clone(),
        })
    }
    fn release_pending(&self) {
        if self.releasing.replace(true) {
            return;
        }
        loop {
            let Some(id) = self.pending.borrow_mut().pop() else {
                break;
            };
            let object = {
                let mut entries = self.entries.borrow_mut();
                entries
                    .get_mut(id.index)
                    .filter(|entry| {
                        entry.generation == id.generation
                            && entry
                                .life
                                .as_ref()
                                .is_some_and(|life| life.references.get() == 0)
                    })
                    .and_then(|entry| {
                        entry.life = None;
                        entry.object.take()
                    })
            };
            // Child-edge drops append to the queue; the arena borrow has ended.
            drop(object);
        }
        self.releasing.set(false);
    }
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "Finalization count is bounded by the finite unmarked object list"
    )]
    pub(crate) fn collect(&self) -> usize {
        self.release_pending();
        let mut unmarked = {
            let entries = self.entries.borrow();
            let mut marked = vec![false; entries.len()];
            let mut todo: Vec<ObjectId> = entries
                .iter()
                .filter_map(|entry| {
                    entry
                        .life
                        .as_ref()
                        .filter(|life| life.external.get() > 0)
                        .map(|life| life.id)
                })
                .collect();
            while let Some(id) = todo.pop() {
                let Some(entry) = entries
                    .get(id.index)
                    .filter(|entry| entry.generation == id.generation)
                else {
                    continue;
                };
                let Some(mark) = marked.get_mut(id.index) else {
                    continue;
                };
                if std::mem::replace(mark, true) {
                    continue;
                }
                if let Some(object) = &entry.object {
                    todo.extend(object.edges().map(|edge| edge.life.id));
                }
            }
            entries
                .iter()
                .zip(marked)
                .filter_map(|(entry, marked)| {
                    if marked {
                        None
                    } else {
                        entry.life.as_ref().map(|life| (life.created, life.id))
                    }
                })
                .collect::<Vec<_>>()
        };
        unmarked.sort_unstable_by_key(|(created, _)| std::cmp::Reverse(*created));
        let mut count = 0;
        for (_, id) in unmarked {
            let object = {
                let mut entries = self.entries.borrow_mut();
                entries
                    .get_mut(id.index)
                    .filter(|entry| entry.generation == id.generation)
                    .and_then(|entry| {
                        let life = entry.life.as_ref()?;
                        Some((Edge::retain(life, false), entry.object.take()))
                    })
            };
            if let Some((guard, object)) = object {
                // Like SQSharedState's temporary reference, retain the current
                // object while Finalize immediately releases outgoing edges.
                drop(object);
                self.release_pending();
                drop(guard);
                self.release_pending();
                count += 1;
            }
        }
        count
    }
}
/// Public owners retain the realm; storage edges never do.
#[derive(Clone, Debug)]
pub(crate) struct Handle {
    // Drop the edge (and drain native-style immediate release) before the realm.
    pub(crate) edge: Edge,
    pub(crate) realm: Realm,
}
impl Handle {
    pub(crate) fn from_edge(edge: &Edge, realm: &Realm) -> Self {
        Self {
            edge: edge.external(),
            realm: realm.clone(),
        }
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.realm.same(&other.realm) && self.edge.life.id == other.edge.life.id
    }
    #[expect(
        clippy::expect_used,
        reason = "A live external edge prevents collection and generation reuse"
    )]
    pub(crate) fn read<T>(&self, read: impl FnOnce(&Object) -> T) -> T {
        let entries = self.realm.heap.entries.borrow();
        let entry = entries
            .get(self.edge.life.id.index)
            .filter(|entry| entry.generation == self.edge.life.id.generation)
            .expect("live object index and generation");
        read(entry.object.as_ref().expect("live object storage"))
    }
    #[expect(
        clippy::expect_used,
        reason = "A live external edge prevents collection and generation reuse"
    )]
    pub(crate) fn mutate<T>(&self, mutate: impl FnOnce(&mut Object) -> T) -> T {
        let result = {
            let mut entries = self.realm.heap.entries.borrow_mut();
            let entry = entries
                .get_mut(self.edge.life.id.index)
                .filter(|entry| entry.generation == self.edge.life.id.generation)
                .expect("live object index and generation");
            mutate(entry.object.as_mut().expect("live object storage"))
        };
        self.realm.heap.release_pending();
        result
    }
    #[cfg(test)]
    pub(crate) fn watch(&self) -> impl Fn() -> Option<Self> + use<> {
        let life = Rc::downgrade(&self.edge.life);
        let realm = self.realm.downgrade();
        move || {
            let life = life.upgrade()?;
            let realm = realm.upgrade()?;
            if life.references.get() == 0 {
                return None;
            }
            Some(Self {
                edge: Edge::retain(&life, true),
                realm,
            })
        }
    }
}

#[cfg(test)]
mod tests;
