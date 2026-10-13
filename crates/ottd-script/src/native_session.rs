//! Materialized native Squirrel session: private-constructor idle graph.
//!
//! This atom materializes the complete initial shared-state graph of the
//! pinned native build (thirteen shared tables, sixty-six default closures,
//! primary context, root table, four base closures) in exact source order,
//! proves its idle root/owner/close order against the original receipts and
//! refuses every execution capability the graph cannot grant yet.
mod base;
mod close;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Session internals are shared across this component's private modules"
)]
mod collector;
mod context;
mod error;
mod friend;
mod friend_init;
mod friend_view;
mod graph;
mod ordinary;
#[expect(
    clippy::redundant_pub_crate,
    reason = "The heap object kind needs the actual context payload storage type"
)]
pub(crate) mod payload;
mod primary;
mod ref_identity;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Session internals are shared across this component's private modules"
)]
mod ref_table;
mod roots;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Session internals are shared across this component's private modules"
)]
mod scalars;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Session internals are shared across this component's private modules"
)]
mod session;
pub use collector::NativeCollector;
#[expect(
    clippy::redundant_pub_crate,
    reason = "The heap dispatch reads the native collection entry across modules"
)]
pub(crate) use collector::collect_shared_state;
pub use error::NativeContextError;
pub use friend::RetainedNativeFriend;
pub use friend_view::{NativeFriendField, NativeFriendScalars, NativeFriendView};
#[expect(
    clippy::redundant_pub_crate,
    reason = "The heap and the Runner-level path read the live-session phase and factory bridge"
)]
pub(crate) use ordinary::{default_root, local_friend, publish_primary_root};
#[expect(
    clippy::redundant_pub_crate,
    reason = "The heap reads the live-session phase of its attached native session"
)]
pub(crate) use session::Phase;
#[expect(
    clippy::redundant_pub_crate,
    reason = "The heap holds a weak hook to the session's shared state"
)]
pub(crate) use session::SharedState;
pub use session::{NativeCapability, NativeSession};

#[cfg(test)]
mod friend_receipt_tests;
#[cfg(test)]
mod friend_tests;
#[cfg(test)]
mod graph_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod observation;
#[cfg(test)]
mod ordinary_tests;
#[cfg(test)]
mod receipt_tests;
#[cfg(test)]
mod receipts;
#[cfg(test)]
mod ref_receipt_tests;
#[cfg(test)]
mod ref_table_tests;
