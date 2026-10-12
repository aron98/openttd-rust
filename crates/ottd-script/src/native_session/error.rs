//! Infrastructure boundary failures of a materialized native session.
//!
//! These are session-infrastructure results, never catchable script exceptions;
//! no variant fabricates a native error string or `VmError` value.

/// Failure of a checked native session allocation or capability request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeContextError {
    /// A checked allocation or reservation of session storage failed.
    Allocation,
    /// The requested initial stack size cannot host a materialized context;
    /// native `sq_open(0)` cannot even complete its base registration pushes.
    InvalidInitialSlots,
    /// The owning session is already closed; no native capability survives it.
    ClosedContext,
    /// The context slot generation no longer matches this capability.
    StaleContext,
    /// The capability belongs to a different native session.
    ForeignContext,
    /// Collection is already running on this session.
    CollectionReentry,
    /// The materialized graph intentionally lacks the requested surface.
    IncompleteNativeGraph,
}
impl From<crate::VmError> for NativeContextError {
    fn from(error: crate::VmError) -> Self {
        // Checked reservations are the only fallible source path while the
        // idle graph is constructed; any other internal refusal is an
        // allocation-infrastructure failure, not a script error.
        let _ = error;
        Self::Allocation
    }
}
