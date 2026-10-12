//! Physical primary-context storage, kept as a real collectable heap object.
use super::scalars::{CallRecord, CallScalars, ContextScalars};
use crate::heap::{Edge, Stored};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Index of the persistent `SQObjectPtr` fields, in native enqueue order
/// (`_lasterror`, `_errorhandler`, `_debughook`, `_roottable`, `temp_reg`).
pub(super) const ERROR: usize = 0;
pub(super) const HANDLER: usize = 1;
pub(super) const DEBUG: usize = 2;
pub(super) const ROOT: usize = 3;
pub(super) const TEMP: usize = 4;

/// Actual `SQVM` storage: source scalar widths, persistent fields, physical
/// stack/vararg/call buffers. Field assignment order differs per native
/// operation, so no single destructor order is imposed on the fields.
#[expect(
    clippy::struct_excessive_bools,
    reason = "The source SQVM member state mirrors its exact SQBool fields"
)]
pub(crate) struct ContextPayload {
    suspended: bool,
    suspended_target: i64,
    suspended_root: bool,
    suspended_traps: i32,
    nnativecalls: i32,
    can_suspend: bool,
    in_stackoverflow: bool,
    ops_till_suspend: i64,
    ops_till_suspend_error_threshold: i64,
    top: i64,
    stackbase: i64,
    callsstacksize: i64,
    etraps: i64,
    persistent: [Stored; 5],
    stack: Vec<Stored>,
    vargs: Vec<Stored>,
    calls: Vec<CallRecord>,
}
impl Debug for ContextPayload {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("ContextPayload")
            .field("suspended", &self.suspended)
            .field("suspended_target", &self.suspended_target)
            .field("suspended_root", &self.suspended_root)
            .field("suspended_traps", &self.suspended_traps)
            .field("nnativecalls", &self.nnativecalls)
            .field("can_suspend", &self.can_suspend)
            .field("in_stackoverflow", &self.in_stackoverflow)
            .field("ops_till_suspend", &self.ops_till_suspend)
            .field(
                "ops_till_suspend_error_threshold",
                &self.ops_till_suspend_error_threshold,
            )
            .field("top", &self.top)
            .field("stackbase", &self.stackbase)
            .field("callsstacksize", &self.callsstacksize)
            .field("etraps", &self.etraps)
            .field("persistent", &self.persistent)
            .field("stack", &self.stack.len())
            .field("vargs", &self.vargs.len())
            .field("calls", &self.calls.len())
            .finish()
    }
}
impl ContextPayload {
    /// The `SQVM` constructor's member defaults; no storage is sized yet.
    pub(crate) const fn allocated() -> Self {
        Self {
            suspended: false,
            suspended_target: -1,
            suspended_root: false,
            suspended_traps: 0,
            nnativecalls: 0,
            can_suspend: false,
            in_stackoverflow: false,
            ops_till_suspend: 0,
            ops_till_suspend_error_threshold: i64::MIN,
            top: 0,
            stackbase: 0,
            callsstacksize: 0,
            etraps: 0,
            persistent: [const { Stored::null() }; 5],
            stack: Vec::new(),
            vargs: Vec::new(),
            calls: Vec::new(),
        }
    }
    /// `SQVM::Init` sizing: the physical stack of null slots and four
    /// allocated call records; the caller publishes the fresh root table.
    pub(crate) fn init(&mut self, initial_slots: usize) -> Result<(), crate::VmError> {
        self.stack
            .try_reserve_exact(initial_slots)
            .map_err(|_| crate::VmError::Allocation)?;
        self.stack.resize(initial_slots, Stored::null());
        self.calls
            .try_reserve_exact(4)
            .map_err(|_| crate::VmError::Allocation)?;
        self.calls.clear();
        self.calls.resize_with(4, CallRecord::allocated);
        self.stackbase = 0;
        self.top = 0;
        self.callsstacksize = 0;
        Ok(())
    }
    /// Publish the root table owner into its persistent field.
    pub(crate) fn publish_root(&mut self, root: Edge) {
        self.persistent[ROOT] = Stored::Table(root);
    }
    /// Scalar view of the source counter state, for idle observations only.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "source scalar state; later atoms read it")
    )]
    pub(crate) fn scalars(&self) -> ContextScalars {
        ContextScalars {
            suspended: self.suspended,
            suspended_target: self.suspended_target,
            suspended_root: self.suspended_root,
            suspended_traps: self.suspended_traps,
            nnativecalls: self.nnativecalls,
            can_suspend: self.can_suspend,
            in_stackoverflow: self.in_stackoverflow,
            ops_till_suspend: self.ops_till_suspend,
            ops_till_suspend_error_threshold: self.ops_till_suspend_error_threshold,
            top: self.top,
            stackbase: self.stackbase,
            callsstacksize: self.callsstacksize,
            etraps: self.etraps,
            stack_size: self.stack.len(),
            vargs_size: self.vargs.len(),
            allocated_calls: self.calls.len(),
        }
    }
    /// Persistent field copies for scalar-only membership observation.
    pub(crate) const fn persistent(&self) -> &[Stored; 5] {
        &self.persistent
    }
    /// Source-default scalars of every allocated call record.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "inactive CallInfo scalars carry no experimental credit"
        )
    )]
    pub(crate) fn call_records(&self) -> impl Iterator<Item = CallScalars> + '_ {
        self.calls.iter().map(|record| record.scalars)
    }
    /// Child edges in `SQVM::EnqueueMarkObjectForChildren` order.
    pub(crate) fn stored_edges(&self) -> impl Iterator<Item = &Edge> {
        self.persistent
            .iter()
            .chain(self.stack.iter())
            .chain(self.vargs.iter())
            .chain(self.calls.iter().map(|record| &record.closure))
            .filter_map(Stored::edge)
    }
    /// `SQVM::Push` with the release-build doubling growth.
    /// # Errors
    /// Returns `Allocation` when the doubled stack cannot be reserved.
    pub(crate) fn push(&mut self, value: Stored) -> Result<(), crate::VmError> {
        let top = usize::try_from(self.top).map_err(|_| crate::VmError::Allocation)?;
        if top >= self.stack.len() {
            let doubled = self
                .stack
                .len()
                .checked_mul(2)
                .ok_or(crate::VmError::Allocation)?;
            self.stack
                .try_reserve(doubled)
                .map_err(|_| crate::VmError::Allocation)?;
            self.stack.resize(doubled, Stored::null());
        }
        let Some(slot) = self.stack.get_mut(top) else {
            return Err(crate::VmError::Allocation);
        };
        *slot = value;
        self.top = top
            .checked_add(1)
            .and_then(|next| i64::try_from(next).ok())
            .ok_or(crate::VmError::Allocation)?;
        Ok(())
    }
    /// `SQVM::Pop`: clear the topmost slots, returning the taken owners in
    /// descending stack order for release after the enclosing heap borrow.
    pub(crate) fn pop(&mut self, count: i64) -> Vec<Stored> {
        let mut released = Vec::new();
        for _ in 0..count {
            let Some(top) = self.top.checked_sub(1) else {
                break;
            };
            let Ok(index) = usize::try_from(top) else {
                break;
            };
            let Some(slot) = self.stack.get_mut(index) else {
                break;
            };
            released.push(std::mem::replace(slot, Stored::null()));
            self.top = top;
        }
        released
    }
    /// `SQVM::Finalize`: take every owned field piece in source order
    /// (roottable, lasterror, errorhandler, `debughook`, `temp_reg`, call
    /// records, then the complete stack in reverse). Repeated finalize of
    /// already-cleared fields stays valid.
    pub(crate) fn finalize(&mut self) -> Vec<Stored> {
        let mut released = Vec::new();
        for (field, slot) in [ROOT, ERROR, HANDLER, DEBUG, TEMP]
            .into_iter()
            .zip(&mut self.persistent)
        {
            let _ = field;
            released.push(std::mem::replace(slot, Stored::null()));
        }
        for record in &mut self.calls {
            released.push(std::mem::replace(&mut record.closure, Stored::null()));
        }
        while let Some(slot) = self.stack.pop() {
            released.push(slot);
        }
        released
    }
}
