//! Native marking order and unmarked-object finalization.
use super::ownership::Ownership;
use super::{Edge, Entry, Handle, Heap, Object, ObjectId};
use crate::Realm;

enum Finalization {
    Table,
    Sequence,
    Context,
    Released(Object),
}

impl Heap {
    pub(crate) fn collect(&self) -> usize {
        let roots = self.external_ids();
        // A live materialized session runs its source-ordered native
        // collection, which also retains the legacy external root class of
        // the Runner-level path (`heap/collection.rs` external edges stand
        // in for the frame writers of the later slot-binding atom).
        if let Some(state) = self.native_session() {
            return crate::native_session::collect_shared_state(&state);
        }
        self.collect_roots(&roots)
    }
    /// External host-root identities of the legacy collection root class.
    pub(crate) fn external_ids(&self) -> Vec<ObjectId> {
        let entries = self.entries.borrow();
        let mut roots = Vec::new();
        for entry in &*entries {
            if let Some(life) = &entry.life {
                if life.external.get() > 0 {
                    roots.push(life.id);
                }
            }
        }
        roots
    }
    /// The live materialized session attached to this realm's heap, if any;
    /// a closed or dropped session upgrades to nothing.
    pub(crate) fn native_session(&self) -> Option<std::rc::Rc<crate::native_session::SharedState>> {
        let native = self.native.borrow();
        let state = native.as_ref()?.upgrade()?;
        (state.phase() == crate::native_session::Phase::Live).then_some(state)
    }
    /// One external host handle for the live object at a numeric identity.
    pub(crate) fn handle_at(&self, realm: &Realm, id: ObjectId) -> Option<Handle> {
        let entries = self.entries.borrow();
        let entry = entries
            .get(id.index)
            .filter(|entry| entry.generation == id.generation)?;
        let life = entry.life.as_ref()?;
        Some(Handle {
            edge: Edge::retain(life, Ownership::HostRoot),
            realm: realm.clone(),
        })
    }
    /// Collect with the given source-ordered root identities, mirroring
    /// native `SQSharedState::CollectGarbage`: marking happens at enqueue,
    /// the chain is rebuilt from the LIFO pop order and unmarked nodes
    /// finalize in chain head order.
    pub(crate) fn collect_native(&self, roots: &[ObjectId]) -> usize {
        self.collect_roots(roots)
    }
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "Finalization count is bounded by the finite unmarked object list"
    )]
    fn collect_roots(&self, roots: &[ObjectId]) -> usize {
        self.release_pending();
        let (unmarked, mut reachable) = {
            let entries = self.entries.borrow();
            let mut marked = vec![false; entries.len()];
            let mut todo = Vec::new();
            for id in roots {
                Self::enqueue(&entries, &mut marked, &mut todo, *id);
            }
            let mut reachable = Vec::new();
            while let Some(id) = todo.pop() {
                if let Some(object) = entries
                    .get(id.index)
                    .and_then(|entry| entry.object.as_deref())
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
                        let released = match entry.object.as_deref_mut()? {
                            Object::ScriptClosure(data) => Finalization::Released(Object::Array(
                                std::mem::take(&mut data.outers),
                            )),
                            Object::Table(_) => Finalization::Table,
                            Object::Array(_) | Object::NativeClosure(_) => Finalization::Sequence,
                            Object::Context(_) => Finalization::Context,
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
                    Finalization::Context => self.finalize_context(id),
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
                    .and_then(|entry| match entry.object.as_deref_mut()? {
                        Object::Table(data) => data.nodes.finalize_slot(slot),
                        Object::Array(_)
                        | Object::NativeClosure(_)
                        | Object::ScriptClosure(_)
                        | Object::Context(_) => None,
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
                .and_then(|entry| match entry.object.as_deref_mut()? {
                    Object::Table(data) => data.delegate.take(),
                    Object::Array(_)
                    | Object::NativeClosure(_)
                    | Object::ScriptClosure(_)
                    | Object::Context(_) => None,
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
                    let values = match entry.object.as_deref_mut()? {
                        Object::Array(values) => values,
                        Object::NativeClosure(data) => &mut data.outer_values,
                        Object::Table(_) | Object::ScriptClosure(_) | Object::Context(_) => {
                            return None;
                        }
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
    /// Finalize the table targeted by this edge, releasing slots in source
    /// order for the explicit `Finalize` calls of the shared close sequence.
    pub(crate) fn finalize_table_edge(&self, edge: &Edge) {
        self.finalize_table(edge.identity());
    }
    /// Finalize a context exactly like `SQVM::Finalize`, releasing the payload    /// pieces taken under a short borrow one by one afterward.
    fn finalize_context(&self, id: ObjectId) {
        let released = {
            let mut entries = self.entries.borrow_mut();
            entries
                .get_mut(id.index)
                .and_then(|entry| match entry.object.as_deref_mut()? {
                    Object::Context(payload) => Some(payload.finalize()),
                    Object::Array(_)
                    | Object::NativeClosure(_)
                    | Object::ScriptClosure(_)
                    | Object::Table(_) => None,
                })
                .unwrap_or_default()
        };
        for piece in released {
            drop(piece);
            self.release_pending();
        }
    }
    fn enqueue(entries: &[Entry], marked: &mut [bool], todo: &mut Vec<ObjectId>, id: ObjectId) {
        if entries
            .get(id.index)
            .is_some_and(|entry| entry.generation == id.generation && entry.life.is_some())
        {
            if let Some(mark) = marked.get_mut(id.index) {
                if !std::mem::replace(mark, true) {
                    todo.push(id);
                }
            }
        }
    }
}
