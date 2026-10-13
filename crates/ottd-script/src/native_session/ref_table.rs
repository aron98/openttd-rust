//! Source-faithful physical `RefTable` storage and algorithm
//! (`sqstate.cpp:367-515`).
//!
//! Each node owns one real object reference like the native `RefNode`'s
//! `SQObjectPtr`: a slot store is an owning `Stored` value plus its cached
//! raw identity, so repeated `AddRef` only increments the node handle count
//! and the final `Release` drops the owning reference after unlinking.
//! Keys hash and compare by the native `HashObj` rules (`ref_identity.rs`).
mod ops;

use super::error::NativeContextError;
use super::ref_identity::RawSlot;
use crate::heap::Stored;

/// One occupied physical slot: the owning object and its raw identity key.
#[derive(Clone, Debug)]
struct RefSlot {
    object: Stored,
    key: RawSlot,
}
/// One physical `RefNode` (`sqstate.h:50-56`): an owning object slot, its
/// handle refcount and the next link; free nodes chain through `next`.
#[derive(Clone, Debug)]
struct RefNode {
    slot: Option<RefSlot>,
    refs: u64,
    next: Option<usize>,
}
/// The physical reference table: four initial slots/buckets, an ascending
/// free chain, doubling resize and u64 counters, exactly as in the source.
#[derive(Debug)]
pub(crate) struct RefTable {
    numofslots: u64,
    slotused: u64,
    nodes: Vec<RefNode>,
    buckets: Vec<Option<usize>>,
    freelist: Option<usize>,
}
#[expect(
    clippy::indexing_slicing,
    reason = "Bucket and node indexes are masked to the power-of-two slot count"
)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Power-of-two slot counts and valid chain indexes bound all arithmetic"
)]
impl RefTable {
    /// Construct with the source's four initial physical slots and buckets
    /// (`sqstate.cpp:367-369 RefTable::RefTable` -> `AllocNodes(4)`).
    /// # Errors
    /// Returns `Allocation` if the initial storage cannot be reserved.
    pub(crate) fn new() -> Result<Self, NativeContextError> {
        let mut table = Self {
            numofslots: 0,
            slotused: 0,
            nodes: Vec::new(),
            buckets: Vec::new(),
            freelist: None,
        };
        table.alloc_nodes(4)?;
        Ok(table)
    }
    /// Allocated physical slot count.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "physical owner state; the idle snapshot reads it")
    )]
    pub(crate) const fn numofslots(&self) -> u64 {
        self.numofslots
    }
    /// Number of physical slots currently holding one object.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "physical owner state; the idle snapshot reads it")
    )]
    pub(crate) const fn slotused(&self) -> u64 {
        self.slotused
    }
    /// `RefTable::AllocNodes` (`sqstate.cpp:489-515`): one storage block of
    /// null slots whose free chain ascends from slot zero.
    fn alloc_nodes(&mut self, size: u64) -> Result<(), NativeContextError> {
        let Ok(size) = usize::try_from(size) else {
            return Err(NativeContextError::Allocation);
        };
        let mut nodes = Vec::new();
        let mut buckets = Vec::new();
        nodes
            .try_reserve_exact(size)
            .map_err(|_| NativeContextError::Allocation)?;
        buckets
            .try_reserve_exact(size)
            .map_err(|_| NativeContextError::Allocation)?;
        nodes.resize_with(size, || RefNode {
            slot: None,
            refs: 0,
            next: None,
        });
        let mut next = None;
        for (index, node) in nodes.iter_mut().enumerate().rev() {
            node.next = next;
            next = Some(index);
        }
        buckets.resize(size, None);
        self.nodes = nodes;
        self.buckets = buckets;
        self.freelist = Some(0);
        self.slotused = 0;
        self.numofslots = size as u64;
        Ok(())
    }
    /// Main hash bucket of one key (`HashObj(key) & (_numofslots-1)`).
    fn main_position(&self, key: RawSlot) -> usize {
        usize::try_from(key.hash() & (self.numofslots - 1)).unwrap_or_default()
    }
    /// `RefTable::Get` (`sqstate.cpp:467-487`): walk the hash bucket chain
    /// comparing raw value and exact type; the found node, its chain
    /// predecessor and the main position come back together, and a missing
    /// key is added when requested — doubling a full table first and
    /// recomputing the main position like the source.
    fn get(
        &mut self,
        key: RawSlot,
        add: bool,
    ) -> Result<(Option<usize>, Option<usize>, usize), NativeContextError> {
        let mut mainpos = self.main_position(key);
        let mut prev = None;
        let mut found = self.buckets[mainpos];
        while let Some(index) = found {
            let Some(node) = self.nodes.get(index) else {
                break;
            };
            if let Some(slot) = &node.slot {
                if slot.key == key {
                    break;
                }
            }
            prev = found;
            found = node.next;
        }
        if found.is_none() && add {
            if self.numofslots == self.slotused {
                let doubled = self
                    .numofslots
                    .checked_mul(2)
                    .ok_or(NativeContextError::Allocation)?;
                self.resize(doubled)?;
                mainpos = self.main_position(key);
            }
            found = Some(self.add_node(mainpos));
        }
        Ok((found, prev, mainpos))
    }
    /// `RefTable::Add` (`sqstate.cpp:454-465`): take the first free slot,
    /// store the object, prepend it to its bucket and advance the free chain.
    fn add_node(&mut self, mainpos: usize) -> usize {
        #[expect(
            clippy::expect_used,
            reason = "Get doubles a full table before Add, so a free node exists"
        )]
        let index = self
            .freelist
            .expect("freelist node after full-table resize");
        let next = self.nodes.get(index).and_then(|node| node.next);
        if let Some(node) = self.nodes.get_mut(index) {
            node.next = self.buckets[mainpos];
        }
        self.buckets[mainpos] = Some(index);
        self.freelist = next;
        self.slotused += 1;
        index
    }
    /// `RefTable::AddRef` (`sqstate.cpp:402-406`): a repeated handle only
    /// increments the node's refcount; the passed owning object is stored
    /// for a fresh node and otherwise released unchanged.
    /// # Errors
    /// Returns `Allocation` if a full table cannot double its storage.
    pub(super) fn add_ref(
        &mut self,
        object: Stored,
        key: RawSlot,
    ) -> Result<(), NativeContextError> {
        if let (Some(index), _, _) = self.get(key, true)? {
            if let Some(node) = self.nodes.get_mut(index) {
                if node.slot.is_none() {
                    node.slot = Some(RefSlot { object, key });
                }
                node.refs = node.refs.saturating_add(1);
                return Ok(());
            }
        }
        drop(object);
        Ok(())
    }
}
