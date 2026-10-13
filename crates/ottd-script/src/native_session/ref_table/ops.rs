//! `RefTable` release, growth and close operations (`sqstate.cpp:372-452`).
use super::super::error::NativeContextError;
use super::super::ref_identity::{OT_STRING, RawSlot};
use super::{RefSlot, RefTable};
use crate::heap::{ObjectId, Stored};

#[expect(
    clippy::indexing_slicing,
    reason = "Bucket and node indexes are masked to the power-of-two slot count"
)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Power-of-two slot counts and valid chain indexes bound all arithmetic"
)]
impl RefTable {
    /// `RefTable::Release` (`sqstate.cpp:409-428`): decrement the handle
    /// refcount; a final release first retains the object temporarily, then
    /// unlinks the bucket node, prepends the freed slot to the free chain,
    /// decrements the used count, nulls the slot owner and drops the
    /// temporary at function exit. An absent key reports `false` like the
    /// source's failed assertion path.
    /// # Errors
    /// Returns `Allocation` on resize failure of a preceding lookup.
    pub(in crate::native_session) fn release(
        &mut self,
        key: RawSlot,
    ) -> Result<bool, NativeContextError> {
        let (found, prev, mainpos) = self.get(key, false)?;
        let Some(index) = found else {
            return Ok(false);
        };
        let Some(node) = self.nodes.get_mut(index) else {
            return Ok(false);
        };
        node.refs = node.refs.saturating_sub(1);
        if node.refs > 0 {
            return Ok(false);
        }
        // SQObjectPtr o = ref->obj: hold the owner through the unlink.
        let temporary = node.slot.take();
        let next = node.next;
        if let Some(prev) = prev {
            if let Some(previous) = self.nodes.get_mut(prev) {
                previous.next = next;
            }
        } else {
            self.buckets[mainpos] = next;
        }
        if let Some(node) = self.nodes.get_mut(index) {
            node.next = self.freelist;
        }
        self.freelist = Some(index);
        self.slotused -= 1;
        // ref->obj = _null_ dropped the owner when the slot was taken; the
        // temporary's release happens here at function exit.
        drop(temporary);
        Ok(true)
    }
    /// Current handle refcount of one node, by raw identity.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "physical handle state; the retention witnesses read it"
        )
    )]
    pub(in crate::native_session) fn slot_refs(&self, key: RawSlot) -> Option<u64> {
        self.nodes
            .iter()
            .find(|node| node.slot.as_ref().is_some_and(|slot| slot.key == key))
            .map(|node| node.refs)
    }
    /// Clone the owning reference one node holds, for a repeated retention
    /// of the same object (`sq_addref` on an existing handle).
    pub(in crate::native_session) fn stored_clone(&self, key: RawSlot) -> Option<Stored> {
        self.nodes
            .iter()
            .find(|node| node.slot.as_ref().is_some_and(|slot| slot.key == key))
            .and_then(|node| node.slot.as_ref())
            .map(|slot| slot.object.clone())
    }
    /// `RefTable::Resize` (`sqstate.cpp:430-452`): double the storage, then
    /// re-Add the old physical nodes ascending with their refcounts, null
    /// the old owners and free the old storage.
    pub(super) fn resize(&mut self, size: u64) -> Result<(), NativeContextError> {
        let mut replacement = Self {
            numofslots: 0,
            slotused: 0,
            nodes: Vec::new(),
            buckets: Vec::new(),
            freelist: None,
        };
        replacement.alloc_nodes(size)?;
        let mut old = std::mem::replace(
            self,
            Self {
                numofslots: 0,
                slotused: 0,
                nodes: Vec::new(),
                buckets: Vec::new(),
                freelist: None,
            },
        );
        for node in &mut old.nodes {
            if let Some(RefSlot { object, key }) = node.slot.take() {
                let mainpos = replacement.main_position(key);
                let index = replacement.add_node(mainpos);
                if let Some(moved) = replacement.nodes.get_mut(index) {
                    moved.slot = Some(RefSlot { object, key });
                    moved.refs = node.refs;
                }
            }
        }
        *self = replacement;
        Ok(())
    }
    /// `RefTable::Finalize` (`sqstate.cpp:372-378`): null every stored object
    /// scanning ascending; the caller releases the taken owners after the
    /// table borrow ends. The free chain and counters stay untouched.
    pub(crate) fn finalize(&mut self) -> Vec<Stored> {
        let mut released = Vec::new();
        for node in &mut self.nodes {
            if let Some(RefSlot { object, .. }) = node.slot.take() {
                released.push(object);
            }
        }
        released
    }
    /// `RefTable::EnqueueMarkObject` (`sqstate.cpp:386-396`) in ascending
    /// slot order: every occupied slot enqueues, but a string only occupies
    /// its slot — `SQSharedState::EnqueueMarkObject` skips it
    /// (`sqstate.cpp:280-291`), so no root edge follows.
    pub(in crate::native_session) fn mark_targets(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.nodes.iter().filter_map(|node| {
            node.slot
                .as_ref()
                .filter(|slot| slot.key.tag() != OT_STRING)
                .and_then(|slot| slot.object.edge())
                .map(crate::heap::Edge::identity)
        })
    }
    /// Physical free-chain inspection for owner-algorithm tests.
    #[cfg(test)]
    pub(in crate::native_session) fn free_chain(&self) -> Vec<Option<usize>> {
        let mut chain = Vec::new();
        let mut cursor = self.freelist;
        while let Some(index) = cursor {
            chain.push(Some(index));
            cursor = self.nodes.get(index).and_then(|node| node.next);
        }
        chain.push(None);
        chain
    }
}
