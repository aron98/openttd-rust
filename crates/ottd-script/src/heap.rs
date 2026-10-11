//! Realm-owned objects. Internal edges retain identities without owning the heap.
mod ownership;
mod value;
use crate::{Realm, VmError, table::nodes::Nodes};
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
    Table(Nodes),
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
            .flat_map(Nodes::values)
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
    chain: RefCell<Vec<ObjectId>>,
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
            if object.is_some() {
                self.chain.borrow_mut().retain(|entry| *entry != id);
            }
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
        let (unmarked, mut reachable) = {
            let entries = self.entries.borrow();
            let mut marked = vec![false; entries.len()];
            let mut todo = Vec::new();
            // External owner enumeration is an internal host seam, not a claim
            // about native pointer-hashed RefTable or full VM root ordering.
            for entry in &*entries {
                if let Some(life) = &entry.life {
                    if life.external.get() > 0 {
                        Self::enqueue(&entries, &mut marked, &mut todo, life.id);
                    }
                }
            }
            let mut reachable = Vec::new();
            while let Some(id) = todo.pop() {
                if let Some(object) = entries
                    .get(id.index)
                    .and_then(|entry| entry.object.as_ref())
                {
                    for edge in object.edges() {
                        Self::enqueue(&entries, &mut marked, &mut todo, edge.life.id);
                    }
                }
                reachable.push(id);
            }
            let unmarked = self
                .chain
                .borrow()
                .iter()
                .copied()
                .filter(|id| !marked.get(id.index).copied().unwrap_or(false))
                .collect::<Vec<_>>();
            (unmarked, reachable)
        };
        let mut count = 0;
        for id in unmarked {
            let object = {
                let mut entries = self.entries.borrow_mut();
                entries
                    .get_mut(id.index)
                    .filter(|entry| entry.generation == id.generation)
                    .and_then(|entry| {
                        let life = entry.life.as_ref()?;
                        let guard = Edge::retain(life, false);
                        let released = match entry.object.as_mut()? {
                            Object::ScriptClosure(data) => {
                                Object::Array(std::mem::take(&mut data.outers))
                            }
                            Object::Array(_) | Object::Table(_) | Object::NativeClosure(_) => {
                                entry.object.take()?
                            }
                        };
                        Some((guard, released))
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
        // Native AddToChain prepends each popped object to the next live chain.
        reachable.reverse();
        *self.chain.borrow_mut() = reachable;
        count
    }
    fn enqueue(entries: &[Entry], marked: &mut [bool], todo: &mut Vec<ObjectId>, id: ObjectId) {
        if entries
            .get(id.index)
            .is_some_and(|entry| entry.generation == id.generation && entry.object.is_some())
        {
            if let Some(mark) = marked.get_mut(id.index) {
                if !std::mem::replace(mark, true) {
                    todo.push(id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
