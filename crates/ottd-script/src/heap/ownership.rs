//! Reference edges and unpublished or external ownership handles.
use super::{Heap, Object};
use crate::Realm;
use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ObjectId {
    pub(super) index: usize,
    pub(super) generation: u64,
}
#[derive(Debug)]
pub(super) struct Life {
    pub(super) id: ObjectId,
    pub(super) heap: Weak<Heap>,
    pub(super) references: Cell<usize>,
    pub(super) external: Cell<usize>,
}
/// An ownership edge; its metadata has no strong link to object storage.
#[derive(Debug)]
pub(crate) struct Edge {
    pub(super) life: Rc<Life>,
    external: bool,
}
impl Edge {
    #[cfg(test)]
    pub(crate) fn references(&self) -> usize {
        self.life.references.get()
    }
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "An allocated edge bounds each ownership count below usize::MAX"
    )]
    pub(super) fn retain(life: &Rc<Life>, external: bool) -> Self {
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
/// Native `CLOSURE_OP` links a zero-reference object before capture resolution.
/// Dropping this construction token leaves an orphan for collection; it does not
/// fabricate an owner whose release would eagerly destroy partially set outers.
#[derive(Debug)]
pub(crate) struct Unpublished {
    pub(super) life: Rc<Life>,
    pub(super) realm: Realm,
}
impl Unpublished {
    pub(crate) fn publish(self) -> Handle {
        Handle {
            edge: Edge::retain(&self.life, true),
            realm: self.realm.clone(),
        }
    }
    #[expect(
        clippy::expect_used,
        reason = "The construction token's realm keeps its newly allocated uncollected entry alive"
    )]
    pub(crate) fn mutate<T>(&self, mutate: impl FnOnce(&mut Object) -> T) -> T {
        let result = {
            let mut entries = self.realm.heap.entries.borrow_mut();
            let entry = entries
                .get_mut(self.life.id.index)
                .expect("unpublished object index");
            mutate(entry.object.as_mut().expect("unpublished object storage"))
        };
        self.realm.heap.release_pending();
        result
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
    #[cfg(test)]
    pub(crate) fn references(&self) -> usize {
        self.edge.life.references.get()
    }
    #[cfg(test)]
    pub(crate) fn reference_probe(&self) -> impl Fn() -> Option<usize> + use<> {
        let life = Rc::downgrade(&self.edge.life);
        move || life.upgrade().map(|life| life.references.get())
    }
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
