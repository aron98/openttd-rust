//! Persistent runner payloads keep one owner behind an explicit assignment boundary.
use crate::{
    Realm, RootEnvironment, RootSlot, Value,
    heap::{Edge, Stored},
};
use std::fmt::{Debug, Formatter, Result};

impl<T> RootSlot<T> {
    pub(super) const fn new(value: T) -> Self {
        Self { value }
    }
    pub(super) const fn as_ref(&self) -> &T {
        &self.value
    }
    pub(super) fn set(&mut self, value: T) {
        self.value = value;
    }
}
impl<T: Debug> Debug for RootSlot<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        self.value.fmt(formatter)
    }
}
/// The runner's configured-root slot: it holds the primary's sole root owner
/// and publishes the matching scalar membership of every assignment into the
/// materialized session's primary record, without adding a second owner
/// (design §47, §89).
#[expect(
    clippy::redundant_pub_crate,
    reason = "The crate-root runner and observation modules read the root slot across modules"
)]
#[derive(Debug)]
pub(crate) struct RunnerRoot {
    slot: RootSlot<RootEnvironment>,
    /// The bound collectable context of a local friend; the primary binding
    /// has none and its membership publishes to the session record.
    context: Option<Edge>,
}
impl RunnerRoot {
    pub(crate) const fn new(value: RootEnvironment) -> Self {
        Self {
            slot: RootSlot::new(value),
            context: None,
        }
    }
    pub(crate) const fn as_ref(&self) -> &RootEnvironment {
        self.slot.as_ref()
    }
    /// The bound friend context of this runner, if it has one.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "The friend-field snapshot witness is test-only observation"
        )
    )]
    pub(crate) const fn context(&self) -> Option<&Edge> {
        self.context.as_ref()
    }
    /// Bind a friend's collectable context to this runner's root slot.
    pub(crate) fn bind_context(&mut self, context: Edge) {
        self.context = Some(context);
    }
    /// Assign one configured root and publish its numeric membership first,
    /// exactly like a native `SQObjectPtr` root-field copy: the displaced
    /// owner is released by the slot assignment after the membership read.
    pub(crate) fn set(&mut self, value: RootEnvironment) {
        if self.context.is_none() {
            let realm: &Realm = &self.slot.as_ref().realm;
            let stored = Stored::from_value(Value::Table(value.table.clone()), realm).ok();
            if let (Some(state), Some(stored)) = (realm.heap.native_session(), stored) {
                crate::native_session::publish_primary_root(&state, &stored);
            }
        }
        self.slot.set(value);
    }
}
