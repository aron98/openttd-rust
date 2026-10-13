//! Realm-owned objects. Internal edges retain identities without owning the heap.
mod collection;
mod delegates;
mod operand;
mod ownership;
pub(crate) use operand::Operand;
mod value;
use crate::{Realm, VmError, table::Data};
use ownership::Life;
pub(crate) use ownership::{Edge, Handle, ObjectId, Unpublished};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub(crate) use value::Stored;

#[derive(Debug)]
pub(crate) enum Object {
    ScriptClosure(crate::script_closure::Data),
    NativeClosure(crate::native_closure::Data),
    Table(Data),
    Array(Vec<Stored>),
    Context(Box<crate::native_session::payload::ContextPayload>),
}
impl Object {
    fn edges(&self) -> impl Iterator<Item = &Edge> {
        // Delegate and slot edges are gathered once so a context can yield its
        // source-ordered child edges directly, without a heterogeneous chain.
        let mut edges = Vec::new();
        match self {
            Self::ScriptClosure(data) => {
                edges.extend(data.values().filter_map(Stored::edge));
            }
            Self::Table(nodes) => {
                if let Some(delegate) = nodes.delegate.as_ref() {
                    edges.push(delegate);
                }
                edges.extend(nodes.nodes.values().filter_map(Stored::edge));
            }
            Self::Array(values) => {
                edges.extend(values.iter().filter_map(Stored::edge));
            }
            Self::NativeClosure(data) => {
                edges.extend(data.values().filter_map(Stored::edge));
            }
            Self::Context(payload) => {
                edges.extend(payload.stored_edges());
            }
        }
        edges.into_iter()
    }
}
#[derive(Debug)]
struct Entry {
    generation: u64,
    life: Option<Rc<Life>>,
    /// Stable actual-payload allocation of one object: the box target holds
    /// the object's real storage and never moves while the object lives, so
    /// its address is the native raw-value analogue of an `SQObjectPtr`
    /// (`sqobject.h` `_unVal` pointer member) — unique per live object,
    /// identical for aliases, and ended by actual retirement.
    object: Option<Box<Object>>,
}
#[derive(Debug, Default)]
pub(crate) struct Heap {
    entries: RefCell<Vec<Entry>>,
    pending: RefCell<Vec<ObjectId>>,
    releasing: Cell<bool>,
    chain: RefCell<Vec<ObjectId>>,
    /// Weak hook to the native root enumerator of a materialized session.
    native: RefCell<Option<std::rc::Weak<crate::native_session::SharedState>>>,
    #[cfg(test)]
    retired: RefCell<Option<Vec<ObjectId>>>,
}
impl Heap {
    pub(crate) fn allocate(
        self: &Rc<Self>,
        realm: &Realm,
        object: Object,
    ) -> Result<Handle, VmError> {
        self.unpublished(realm, object).map(Unpublished::publish)
    }
    pub(crate) fn unpublished(
        self: &Rc<Self>,
        realm: &Realm,
        object: Object,
    ) -> Result<Unpublished, VmError> {
        self.chain
            .borrow_mut()
            .try_reserve(1)
            .map_err(|_| VmError::Allocation)?;
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
            heap: Rc::downgrade(self),
            references: Cell::new(0),
            external: Cell::new(0),
        });
        entry.life = Some(Rc::clone(&life));
        entry.object = Some(Box::new(object));
        self.chain.borrow_mut().insert(0, life.id);
        Ok(Unpublished {
            life,
            realm: realm.clone(),
        })
    }
    /// Queue a recorded zero-reference construction allocation for direct
    /// release, as the source constructor's rollback would destroy the raw
    /// prefix; nonzero references leave the entry live.
    pub(crate) fn release_orphan(&self, unpublished: &Unpublished) {
        self.pending.borrow_mut().push(unpublished.life.id);
        self.release_pending();
    }
    /// Attach the weak native root enumerator of one materialized session.
    pub(crate) fn install_native_session(
        &self,
        state: std::rc::Weak<crate::native_session::SharedState>,
    ) {
        *self.native.borrow_mut() = Some(state);
    }
    /// Stable actual-payload address of one live object: the allocation
    /// address that a native `SQObjectPtr` carries as its raw value for
    /// refcounted pointer types. Two live objects never share an address,
    /// every alias of one object reads the same address, and the address
    /// ends at actual retirement (the allocation drops); reuse of a retired
    /// address by a later object is permitted, like the native allocator.
    /// The read adds no owner and mutates no state.
    pub(crate) fn payload_address(&self, id: ObjectId) -> Option<u64> {
        let entries = self.entries.borrow();
        let entry = entries.get(id.index)?;
        if entry.generation != id.generation {
            return None;
        }
        let object = entry.object.as_deref()?;
        u64::try_from(std::ptr::from_ref(object).addr()).ok()
    }
    /// Live owner count of one object, read without adding an owner.
    pub(crate) fn references(&self, id: ObjectId) -> Option<usize> {
        let entries = self.entries.borrow();
        let entry = entries.get(id.index)?;
        if entry.generation != id.generation {
            return None;
        }
        entry.life.as_ref().map(|life| life.references.get())
    }
    /// Used-slot extent of one live table object, read without an owner.
    pub(crate) fn table_used(&self, id: ObjectId) -> Option<usize> {
        let entries = self.entries.borrow();
        let entry = entries.get(id.index)?;
        if entry.generation != id.generation {
            return None;
        }
        match entry.object.as_deref()? {
            Object::Table(data) => Some(data.nodes.len()),
            _ => None,
        }
    }
    /// Take one live object's whole payload allocation out of its arena slot,
    /// leaving the entry alive but empty for a scoped storage lease; the
    /// payload address is unchanged by the move, and `reattach_payload`
    /// restores it allocation-free. The caller must guarantee the leased
    /// object stays marked for the whole lease.
    pub(crate) fn take_payload(&self, id: ObjectId) -> Option<Box<Object>> {
        let mut entries = self.entries.borrow_mut();
        let entry = entries.get_mut(id.index)?;
        if entry.generation != id.generation {
            return None;
        }
        entry.object.take()
    }
    /// Reattach a leased payload allocation to its reserved arena slot.
    pub(crate) fn reattach_payload(&self, id: ObjectId, payload: Box<Object>) {
        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries.get_mut(id.index) {
            if entry.generation == id.generation && entry.life.is_some() {
                debug_assert!(
                    entry.object.is_none(),
                    "payload lease reattached over a live object"
                );
                entry.object = Some(payload);
            }
        }
    }
    /// Run one source-ordered native collection for the attached session, if
    /// its shared state still exists; ordinary legacy realms have none.
    pub(crate) fn collect_native_dispatch(&self) -> Option<usize> {
        let native = self.native.borrow();
        let state = native.as_ref().and_then(std::rc::Weak::upgrade)?;
        Some(crate::native_session::collect_shared_state(&state))
    }
    /// Drain queued zero-reference releases after payload pieces were dropped
    /// outside a heap entry borrow, mirroring immediate `SQObjectPtr` release.
    pub(crate) fn drain_releases(&self) {
        self.release_pending();
    }
    fn release_pending(&self) {
        if self.releasing.replace(true) {
            return;
        }
        loop {
            let Some(id) = self.pending.borrow_mut().pop() else {
                break;
            };
            let retired = {
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
                    .map(|entry| {
                        entry.life = None;
                        entry.object.take()
                    })
            };
            let Some(object) = retired else {
                continue;
            };
            if object.is_some() {
                self.chain.borrow_mut().retain(|entry| *entry != id);
            }
            // Child-edge drops append to the queue; the arena borrow has ended.
            drop(object);
            #[cfg(test)]
            if let Some(trace) = self.retired.borrow_mut().as_mut() {
                assert!(
                    trace.len() < trace.capacity(),
                    "pre-reserved retirement trace overflow"
                );
                trace.push(id);
            }
        }
        self.releasing.set(false);
    }
}

#[cfg(test)]
mod release_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod delegate_tests;
#[cfg(test)]
mod owner_probe;

#[cfg(test)]
mod delegate_lifetimes;

#[cfg(test)]
mod active_owner_tests;

#[cfg(test)]
mod reference_value_tests;

#[cfg(test)]
mod active_root_tests;

#[cfg(test)]
mod operand_tests;

#[cfg(test)]
pub(crate) mod observation;
#[cfg(test)]
mod observation_tests;
