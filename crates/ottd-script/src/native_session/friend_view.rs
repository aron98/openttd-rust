//! The scoped retained-friend view: a storage lease of the friend's stable
//! private payload allocation.
//!
//! The view borrows both the session and the token for the same lifetime, so
//! the token's host retention cannot be released while the view lives. The
//! friend's payload box moves out of its reserved arena slot for the borrow
//! — its actual payload address, identity and logical owners are unchanged —
//! and reattaches allocation-free on drop; the token guarantees the
//! detached-for-borrow context stays marked for collection (design §81).
use super::{error::NativeContextError, friend::RetainedNativeFriend, session::SharedState};
use crate::heap::{Edge, Object, Stored};
use std::rc::Rc;

/// Receipt-shaped idle observation of one retained friend: the native
/// `roots vm` line of the pinned observer plus the kinds and owner counts of
/// its five persistent fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeFriendScalars {
    /// The friend context's live reference count: exactly the one `RefTable`
    /// node owner while the token is retained.
    pub references: usize,
    /// Whether this context is the session's shared `_root_vm`; a friend
    /// never is.
    pub is_root: bool,
    /// Allocated physical stack size of the friend context.
    pub stack_size: usize,
    /// Current stack top index.
    pub top: i64,
    /// Current stack base index.
    pub stackbase: i64,
    /// Allocated physical vararg slot count.
    pub vargs_size: usize,
    /// Active call-record count.
    pub callsstacksize: i64,
    /// Active trap count.
    pub etraps: i64,
    /// Operations remaining until suspension.
    pub ops: i64,
    /// Persistent fields in native order: error, handler, debug, root, temp.
    pub persistent: [NativeFriendField; 5],
}
/// One persistent field of an idle retained friend, with the owner count
/// and extent the observer reports for a table target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFriendField {
    /// A native `OT_NULL` position (`16777217` in the receipts).
    Null,
    /// A table target: its live owner count and used-slot extent.
    Table {
        /// Live references of the target table.
        references: usize,
        /// Used own slots of the target table.
        extent: usize,
    },
}
/// A scoped view of one retained friend; dropping it reattaches the leased
/// payload allocation into its reserved arena slot.
#[derive(Debug)]
pub struct NativeFriendView<'a> {
    state: &'a Rc<SharedState>,
    token: &'a RetainedNativeFriend,
    payload: Option<Box<Object>>,
}
impl NativeFriendView<'_> {
    /// The registry identity of the borrowed friend.
    #[cfg(test)]
    pub(super) const fn context_id(&self) -> super::context::ContextId {
        self.token.context
    }
    /// Copy the friend's scalar idle state for receipt-shaped observation;
    /// the leased payload is read directly and no owner is added.
    #[expect(
        clippy::indexing_slicing,
        reason = "The five persistent fields are a fixed-size array indexed 0..5"
    )]
    pub fn scalars(&self) -> NativeFriendScalars {
        let Some(payload) = self.payload.as_deref() else {
            return null_scalars();
        };
        let Object::Context(data) = payload else {
            return null_scalars();
        };
        let scalars = data.scalars();
        let heap = &self.state.realm.heap;
        // Idle friend fields hold only the copied roottable or Null.
        let persistent = std::array::from_fn(|index| match &data.persistent()[index] {
            Stored::Table(edge) => table_field(heap, edge),
            _ => NativeFriendField::Null,
        });
        NativeFriendScalars {
            references: heap.references(self.token.object).unwrap_or(0),
            is_root: false,
            stack_size: scalars.stack_size,
            top: scalars.top,
            stackbase: scalars.stackbase,
            vargs_size: scalars.vargs_size,
            callsstacksize: scalars.callsstacksize,
            etraps: scalars.etraps,
            ops: scalars.ops_till_suspend,
            persistent,
        }
    }
}
/// Classify one table persistent field with its owner count and extent.
fn table_field(heap: &crate::heap::Heap, edge: &Edge) -> NativeFriendField {
    let identity = edge.identity();
    NativeFriendField::Table {
        references: heap.references(identity).unwrap_or(0),
        extent: heap.table_used(identity).unwrap_or(0),
    }
}
const fn null_scalars() -> NativeFriendScalars {
    NativeFriendScalars {
        references: 0,
        is_root: false,
        stack_size: 0,
        top: 0,
        stackbase: 0,
        vargs_size: 0,
        callsstacksize: 0,
        etraps: 0,
        ops: 0,
        persistent: [NativeFriendField::Null; 5],
    }
}
/// Lease the friend's payload for a validated token; the detached-for-borrow
/// context stays marked through the `RefTable` roots.
/// # Errors
/// Returns `Allocation` if the payload cannot be leased.
pub(super) fn lease<'a>(
    state: &'a Rc<SharedState>,
    token: &'a RetainedNativeFriend,
) -> Result<NativeFriendView<'a>, NativeContextError> {
    let payload = state
        .realm
        .heap
        .take_payload(token.object)
        .ok_or(NativeContextError::Allocation)?;
    Ok(NativeFriendView {
        state,
        token,
        payload: Some(payload),
    })
}
impl Drop for NativeFriendView<'_> {
    /// Reattach the leased payload allocation into its reserved arena slot;
    /// the move is allocation-free and unwind-safe, and the payload address
    /// never changed during the lease.
    fn drop(&mut self) {
        if let Some(payload) = self.payload.take() {
            self.state
                .realm
                .heap
                .reattach_payload(self.token.object, payload);
        }
    }
}
