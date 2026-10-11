//! Realm-owned objects. Internal edges retain identities without owning the heap.
mod collection;
mod delegates;
mod ownership;
mod value;
use crate::{Realm, VmError, table::Data};
pub(crate) use ownership::{Edge, Handle, Unpublished};
use ownership::{Life, ObjectId};
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
}
impl Object {
    fn edges(&self) -> impl Iterator<Item = &Edge> {
        let (table, array, closure, script) = match self {
            Self::ScriptClosure(data) => (None, None, None, Some(data)),
            Self::Table(nodes) => (Some(nodes), None, None, None),
            Self::Array(values) => (None, Some(values), None, None),
            Self::NativeClosure(data) => (None, None, Some(data), None),
        };
        table
            .into_iter()
            .filter_map(|data| data.delegate.as_ref())
            .chain(
                table
                    .into_iter()
                    .flat_map(|data| data.nodes.values())
                    .chain(array.into_iter().flatten())
                    .chain(
                        closure
                            .into_iter()
                            .flat_map(crate::native_closure::Data::values),
                    )
                    .chain(
                        script
                            .into_iter()
                            .flat_map(crate::script_closure::Data::values),
                    )
                    .filter_map(Stored::edge),
            )
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
    chain: RefCell<Vec<ObjectId>>,
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
        entry.object = Some(object);
        self.chain.borrow_mut().insert(0, life.id);
        Ok(Unpublished {
            life,
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
