//! Reference edges and unpublished or external ownership handles.
use super::{Heap, Object};
use crate::{Realm, VmError};
use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ObjectId {
    pub(crate) index: usize,
    pub(crate) generation: u64,
}
#[derive(Debug)]
pub(super) struct Life {
    pub(super) id: ObjectId,
    pub(super) heap: Weak<Heap>,
    pub(super) references: Cell<usize>,
    pub(super) external: Cell<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ownership {
    HostRoot,
    Reference,
    Stored,
}
/// An ownership edge; its metadata has no strong link to object storage.
#[derive(Debug)]
pub(crate) struct Edge {
    pub(super) life: Rc<Life>,
    ownership: Ownership,
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
    pub(super) fn retain(life: &Rc<Life>, ownership: Ownership) -> Self {
        life.references.set(life.references.get() + 1);
        if ownership == Ownership::HostRoot {
            life.external.set(life.external.get() + 1);
        }
        Self {
            life: Rc::clone(life),
            ownership,
        }
    }
    pub(crate) fn internal(&self) -> Self {
        Self::retain(&self.life, Ownership::Stored)
    }
    /// Numeric arena identity of this edge's target; this carries no claim
    /// about native pointer identity.
    pub(crate) fn identity(&self) -> ObjectId {
        self.life.id
    }
    pub(crate) fn make_reference(&mut self) -> Result<(), VmError> {
        if self.ownership == Ownership::HostRoot {
            let roots = self
                .life
                .external
                .get()
                .checked_sub(1)
                .ok_or(VmError::InvalidBytecode)?;
            self.life.external.set(roots);
        }
        self.ownership = Ownership::Reference;
        Ok(())
    }
    fn external(&self) -> Self {
        Self::retain(&self.life, Ownership::HostRoot)
    }
}
impl Clone for Edge {
    fn clone(&self) -> Self {
        Self::retain(&self.life, self.ownership)
    }
}
impl Drop for Edge {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "Each edge owns one live reference, and external edges one external reference"
    )]
    fn drop(&mut self) {
        let life = &self.life;
        if self.ownership == Ownership::HostRoot {
            life.external.set(life.external.get() - 1);
        }
        life.references.set(life.references.get() - 1);
        if let Some(heap) = life.heap.upgrade() {
            if life.references.get() == 0 {
                heap.pending.borrow_mut().push(life.id);
            }
            if self.ownership != Ownership::Stored {
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
            edge: Edge::retain(&self.life, Ownership::HostRoot),
            realm: self.realm.clone(),
        }
    }
    /// Clone this construction token for guard bookkeeping; the share owns no
    /// reference and only records the zero-ref allocation for rollback drains.
    pub(crate) fn share(&self) -> Self {
        Self {
            life: Rc::clone(&self.life),
            realm: self.realm.clone(),
        }
    }
    /// Retain the single owner a source `SQObjectPtr` field assignment adds to
    /// this raw zero-reference allocation; the owner drains releases directly
    /// instead of deferring them like storage-internal edges.
    pub(crate) fn publish_shared(self) -> Edge {
        Edge::retain(&self.life, Ownership::Reference)
    }
    /// Retain one internal storage owner for assignment into another object's
    /// slot or field; the enclosing release path drains it later.
    pub(crate) fn stored(self) -> Edge {
        Edge::retain(&self.life, Ownership::Stored)
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
#[derive(Debug)]
pub(crate) struct Handle {
    // Drop the edge (and drain native-style immediate release) before the realm.
    pub(crate) edge: Edge,
    pub(crate) realm: Realm,
}
impl Clone for Handle {
    fn clone(&self) -> Self {
        Self {
            edge: self.edge.external(),
            realm: self.realm.clone(),
        }
    }
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
        reason = "A live owning edge prevents retirement and generation reuse; finalizers retain cleared storage"
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
        reason = "A live owning edge prevents retirement and generation reuse; finalizers retain cleared storage"
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
                edge: Edge::retain(&life, Ownership::HostRoot),
                realm,
            })
        }
    }
}
