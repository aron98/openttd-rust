//! Source-faithful physical `RefTable` storage and algorithm.
//!
//! Host retention through this table is refused by the idle session boundary
//! (`NativeCapability::NonEmptyRefTable`), so a materialized idle session can
//! never populate it; the physical layout, ascending free chain, doubling
//! resize and ascending mark/finalize scans are the implemented source
//! algorithm from `sqstate.cpp`, pending the stable payload-identity
//! prerequisite that must supply real slot objects and native hash/equality.
use super::error::NativeContextError;

/// Neutral slot token: an exact raw identity plus its native type tag. A
/// future stable-identity atom replaces this with real native handles; no
/// pointer identity or hash parity is claimed for the token itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawSlot {
    raw: u64,
    tag: u32,
}
impl RawSlot {
    /// Construct a neutral token; real native handles arrive with the
    /// identity prerequisite.
    #[cfg(test)]
    pub(super) const fn new(raw: u64, tag: u32) -> Self {
        Self { raw, tag }
    }
    /// Source pointer-path main-position hash arithmetic (raw value shift 3).
    const fn hash(self) -> u64 {
        self.raw >> 3
    }
}
/// One physical `RefNode`: a single owning object plus its handle refcount
/// and hash-chain link; the free chain reuses node indexes ascending.
#[derive(Clone, Debug)]
struct RefNode {
    object: Option<RawSlot>,
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
    /// Construct with the source's four initial physical slots and buckets.
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
            object: None,
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
    fn main_position(&self, token: RawSlot) -> usize {
        usize::try_from(token.hash() & (self.numofslots - 1)).unwrap_or_default()
    }
    /// `RefTable::Get`: walk the hash bucket chain comparing the exact raw
    /// value and type tag; the add path first doubles a full table.
    pub(super) fn get(
        &mut self,
        token: RawSlot,
        add: bool,
    ) -> Result<Option<usize>, NativeContextError> {
        let mut mainpos = self.main_position(token);
        let mut found = self.buckets[mainpos];
        while let Some(index) = found {
            let Some(node) = self.nodes.get(index) else {
                break;
            };
            if node.object == Some(token) {
                break;
            }
            found = node.next;
        }
        if found.is_none() && add {
            if self.numofslots == self.slotused {
                let doubled = self
                    .numofslots
                    .checked_mul(2)
                    .ok_or(NativeContextError::Allocation)?;
                self.resize(doubled)?;
                mainpos = self.main_position(token);
            }
            found = Some(self.add_node(mainpos, token));
        }
        Ok(found)
    }
    /// `RefTable::Add`: take the first free slot, prepend it to its bucket
    /// and advance the ascending free chain.
    fn add_node(&mut self, mainpos: usize, token: RawSlot) -> usize {
        let index = self.freelist.unwrap_or_default();
        let next = self.nodes.get(index).and_then(|node| node.next);
        if let Some(node) = self.nodes.get_mut(index) {
            node.object = Some(token);
            node.next = self.buckets[mainpos];
        }
        self.buckets[mainpos] = Some(index);
        self.freelist = next;
        self.slotused += 1;
        index
    }
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "physical host-retention algorithm; the idle session boundary refuses populating the RefTable"
        )
    )]
    /// `RefTable::AddRef`: repeated retention of one raw object only
    /// increments the node's handle refcount.
    /// # Errors
    /// Returns `Allocation` if a full table cannot double its storage.
    pub(super) fn add_ref(&mut self, token: RawSlot) -> Result<(), NativeContextError> {
        if let Some(index) = self.get(token, true)? {
            if let Some(node) = self.nodes.get_mut(index) {
                node.refs = node.refs.saturating_add(1);
            }
        }
        Ok(())
    }
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "physical host-retention algorithm; the idle session boundary refuses populating the RefTable"
        )
    )]
    /// `RefTable::Release`: decrement the handle refcount; a final release
    /// retains the object temporarily, unlinks the bucket node, prepends the
    /// freed slot to the free chain and drops the temporary at function exit.
    /// # Errors
    /// Returns `Allocation` on resize failure; absence reports `false` like
    /// the source's failed assertion path.
    pub(super) fn release(&mut self, token: RawSlot) -> Result<bool, NativeContextError> {
        let Some(index) = self.get(token, false)? else {
            return Ok(false);
        };
        let Some(node) = self.nodes.get_mut(index) else {
            return Ok(false);
        };
        node.refs = node.refs.saturating_sub(1);
        if node.refs > 0 {
            return Ok(false);
        }
        node.object = None;
        let next = node.next;
        node.next = self.freelist;
        self.freelist = Some(index);
        let mainpos = self.main_position(token);
        if self.buckets[mainpos] == Some(index) {
            self.buckets[mainpos] = next;
        } else {
            let mut cursor = self.buckets[mainpos];
            while let Some(candidate) = cursor {
                let Some(node) = self.nodes.get_mut(candidate) else {
                    break;
                };
                if node.next == Some(index) {
                    node.next = next;
                    break;
                }
                cursor = node.next;
            }
        }
        self.slotused -= 1;
        Ok(true)
    }
    /// `RefTable::Resize`: double the storage, then re-Add the old physical
    /// nodes ascending with their refcounts, null the old owners and free the
    /// old storage.
    fn resize(&mut self, size: u64) -> Result<(), NativeContextError> {
        let mut replacement = Self {
            numofslots: 0,
            slotused: 0,
            nodes: Vec::new(),
            buckets: Vec::new(),
            freelist: None,
        };
        replacement.alloc_nodes(size)?;
        for node in &self.nodes {
            if let Some(token) = node.object {
                let mainpos = replacement.main_position(token);
                let index = replacement.add_node(mainpos, token);
                if let Some(moved) = replacement.nodes.get_mut(index) {
                    moved.refs = node.refs;
                }
            }
        }
        *self = replacement;
        Ok(())
    }
    /// `RefTable::Finalize`: null every stored object, scanning ascending;
    /// the source leaves the free chain and counters untouched.
    pub(crate) fn finalize(&mut self) {
        for node in &mut self.nodes {
            node.object = None;
        }
    }
    /// `RefTable::EnqueueMarkObject`: retainable objects in ascending slot
    /// order; the idle table holds none, so no root edge follows the scan.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "physical root class; the identity atom binds real handles"
        )
    )]
    pub(super) fn mark_objects(&self) -> impl Iterator<Item = RawSlot> + '_ {
        self.nodes.iter().filter_map(|node| node.object)
    }
    /// Physical free-chain inspection for owner-algorithm tests.
    #[cfg(test)]
    pub(super) fn free_chain(&self) -> Vec<Option<usize>> {
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
