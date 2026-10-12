//! Native marking order and unmarked-object finalization.
use super::ownership::Ownership;
use super::{Edge, Entry, Heap, Object, ObjectId};

enum Finalization {
    Table,
    Sequence,
    Released(Object),
}

impl Heap {
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
                        let guard = Edge::retain(life, Ownership::Stored);
                        let released = match entry.object.as_mut()? {
                            Object::ScriptClosure(data) => Finalization::Released(Object::Array(
                                std::mem::take(&mut data.outers),
                            )),
                            Object::Table(_) => Finalization::Table,
                            Object::Array(_) | Object::NativeClosure(_) => Finalization::Sequence,
                        };
                        Some((guard, released))
                    })
            };
            if let Some((guard, object)) = object {
                // Like SQSharedState's temporary reference, retain the current
                // object while Finalize immediately releases outgoing edges.
                match object {
                    Finalization::Table => self.finalize_table(id),
                    Finalization::Sequence => self.finalize_sequence(id),
                    Finalization::Released(object) => drop(object),
                }
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
    fn finalize_table(&self, id: ObjectId) {
        for slot in 0.. {
            let released = {
                let mut entries = self.entries.borrow_mut();
                entries
                    .get_mut(id.index)
                    .and_then(|entry| match entry.object.as_mut()? {
                        Object::Table(data) => data.nodes.finalize_slot(slot),
                        Object::Array(_) | Object::NativeClosure(_) | Object::ScriptClosure(_) => {
                            None
                        }
                    })
            };
            let Some((key, value)) = released else {
                break;
            };
            drop(key);
            drop(value);
            self.release_pending();
        }
        let delegate = {
            let mut entries = self.entries.borrow_mut();
            entries
                .get_mut(id.index)
                .and_then(|entry| match entry.object.as_mut()? {
                    Object::Table(data) => data.delegate.take(),
                    Object::Array(_) | Object::NativeClosure(_) | Object::ScriptClosure(_) => None,
                })
        };
        drop(delegate);
        self.release_pending();
    }
    fn finalize_sequence(&self, id: ObjectId) {
        for slot in 0.. {
            let released = {
                let mut entries = self.entries.borrow_mut();
                entries.get_mut(id.index).and_then(|entry| {
                    let values = match entry.object.as_mut()? {
                        Object::Array(values) => values,
                        Object::NativeClosure(data) => &mut data.outer_values,
                        Object::Table(_) | Object::ScriptClosure(_) => return None,
                    };
                    if slot == values.len() {
                        values.clear();
                        return None;
                    }
                    values
                        .get_mut(slot)
                        .map(|value| std::mem::replace(value, super::Stored::null()))
                })
            };
            let Some(value) = released else {
                break;
            };
            drop(value);
            self.release_pending();
        }
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
