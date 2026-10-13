//! Numeric context identities, lifecycle and registry records.
use crate::heap::{ObjectId, Stored};
use std::cell::Cell;

/// Identifies one materialized context inside its owning session; this is a
/// numeric session-arena identity, never a table surrogate or Runner address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ContextId {
    pub(super) index: usize,
    pub(super) generation: u64,
}
/// Lifecycle of one context; finalize and reference retirement are separate
/// source steps, so the phase distinguishes them explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContextPhase {
    /// Storage allocated, no Init yet.
    Allocated,
    /// Init is sizing buffers and publishing the root owner.
    Initializing,
    /// Fully initialized idle context.
    Live,
    /// Finalized while the shared `_root_vm` reference is still live.
    FinalizedReferenceLive,
    /// The last logical reference retired the actual storage.
    Retired,
}
/// Logical native refcount and retirement state of one context, tracked
/// separately from heap storage edges.
#[derive(Debug)]
pub(super) struct ContextLife {
    pub(super) references: Cell<u64>,
    pub(super) phase: Cell<ContextPhase>,
}
impl ContextLife {
    pub(super) const fn new(phase: ContextPhase) -> Self {
        Self {
            references: Cell::new(0),
            phase: Cell::new(phase),
        }
    }
    /// Current lifecycle phase.
    pub(super) fn phase(&self) -> ContextPhase {
        self.phase.get()
    }
    pub(super) fn set_phase(&self, phase: ContextPhase) {
        self.phase.set(phase);
    }
    /// Retain one logical native reference, like an `SQObjectPtr` assignment.
    pub(super) fn retain(&self) {
        self.references.set(self.references.get().saturating_add(1));
    }
    /// Release one logical reference; the result reports reference liveness.
    pub(super) fn release(&self) -> bool {
        self.references.set(self.references.get().saturating_sub(1));
        self.references.get() == 0
    }
    /// Current logical refcount.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "idle observation state; consuming atoms read it")
    )]
    pub(super) fn references(&self) -> u64 {
        self.references.get()
    }
}
/// What one persistent slot or field targets, held numerically only; the
/// registry owns no Value mirrors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RootMembership {
    /// A collectable object target.
    Object(ObjectId),
    /// A materialized context target.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "context slot targets arrive with the friend-context atom"
        )
    )]
    Context(ContextId),
    /// A refcounted but non-collectable scalar (string) target.
    NonCollectable,
    /// An explicit native Null position.
    Null,
}
impl RootMembership {
    /// Classify one stored field piece without adding any owner; a heap edge
    /// target is reported as its numeric object identity. A context edge
    /// carries its heap object identity at storage level; registry-level
    /// context membership is resolved by the session records, not by
    /// storage classification.
    pub(super) fn of(stored: &Stored) -> Self {
        match stored {
            Stored::Table(edge)
            | Stored::Array(edge)
            | Stored::NativeClosure(edge)
            | Stored::ScriptClosure(edge)
            | Stored::Context(edge) => Self::Object(edge.identity()),
            Stored::Scalar(crate::table::scalar::Scalar::Null) => Self::Null,
            Stored::Scalar(_) => Self::NonCollectable,
        }
    }
}
/// Storage kind of one context; the primary is protected by `_root_vm`, a
/// retained friend by its explicit `RefTable` host retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContextStorage {
    /// The session's primary context, retained once by `_root_vm`.
    Primary,
    /// A friend context retained through the physical `RefTable`
    /// (`sqapi.cpp:155 sq_addref`).
    Friend,
}
/// One slot of the session's context arena. A retired record's slot keeps
/// its generation counter, so reuse of the slot increments the identity
/// generation without ever wrapping; a slot whose generation reaches
/// `u64::MAX` stays permanently unavailable (design §45).
#[derive(Debug)]
pub(super) struct ContextSlot {
    next_generation: std::cell::Cell<u64>,
    record: Option<ContextRecord>,
}
impl ContextSlot {
    pub(super) const fn from_record(record: ContextRecord) -> Self {
        Self {
            next_generation: std::cell::Cell::new(record.id.generation),
            record: Some(record),
        }
    }
    /// Reserve the next identity of this slot for a fresh record; occupied
    /// and generation-exhausted slots stay unavailable.
    pub(super) fn reserve(&self, index: usize) -> Option<ContextId> {
        Some(ContextId {
            index,
            generation: self.next_generation.get().checked_add(1)?,
        })
    }
    /// A fresh unoccupied slot whose first reservation gets generation one.
    pub(super) const fn vacant() -> Self {
        Self {
            next_generation: std::cell::Cell::new(0),
            record: None,
        }
    }
    /// Publish a reserved record into its slot.
    pub(super) fn publish(&mut self, record: ContextRecord) {
        self.next_generation.set(record.id.generation);
        self.record = Some(record);
    }
    /// Retire the record, freeing the slot for a later incremented reuse.
    pub(super) const fn retire(&mut self) -> Option<ContextRecord> {
        self.record.take()
    }
    pub(super) const fn record(&self) -> Option<&ContextRecord> {
        self.record.as_ref()
    }
    /// The mutable registry record of an occupied slot.
    pub(super) const fn record_mut(&mut self) -> Option<&mut ContextRecord> {
        self.record.as_mut()
    }
}
/// One registry record: identity, lifecycle, numeric persistent-field
/// memberships and the storage kind.
#[derive(Debug)]
pub(super) struct ContextRecord {
    pub(super) id: ContextId,
    pub(super) life: ContextLife,
    pub(super) storage: ContextStorage,
    pub(super) membership: [RootMembership; 5],
}
impl ContextRecord {
    pub(super) const fn new(
        id: ContextId,
        life: ContextLife,
        storage: ContextStorage,
        membership: [RootMembership; 5],
    ) -> Self {
        Self {
            id,
            life,
            storage,
            membership,
        }
    }
}
