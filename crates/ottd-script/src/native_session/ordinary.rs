//! The Runner-level native-session path: the ordinary default-root
//! construction and bare-local friends materialize through the same factory
//! as `NativeSession::open` (design §133), so every configured root shares
//! the realm's complete initial graph.
use super::{
    context::{ContextSlot, RootMembership},
    error::NativeContextError,
    friend_init::init_from_primary,
    payload,
    session::{SharedState, build_shared_state},
};
use crate::{
    Realm, RootEnvironment, Table, VmError,
    heap::{Edge, Handle, Object, Stored},
};
use std::rc::Rc;

/// Native-backed initial stack size of the primary context (`sq_open` target).
const PRIMARY_STACK: usize = 256;

/// The realm's single primary root through its materialized session: reuse
/// the live session's current root table, or build the complete initial graph
/// and move the primary payload's sole root owner into the bound root slot
/// (design §47) so the root keeps exactly its native one reference.
/// # Errors
/// Returns `Allocation` when a checked construction reservation fails.
#[expect(
    clippy::redundant_pub_crate,
    reason = "The crate-root runner bridge reads the Runner-level factory path across modules"
)]
pub(crate) fn default_root(realm: &Realm) -> Result<RootEnvironment, VmError> {
    if let Some(state) = realm.heap.native_session() {
        let table = primary_root(&state, realm).ok_or(VmError::Allocation)?;
        return Ok(RootEnvironment {
            realm: realm.clone(),
            table,
            session: Some(state),
        });
    }
    let state = build_shared_state(realm, PRIMARY_STACK).map_err(|_| VmError::Allocation)?;
    let table = take_payload_root(&state, realm).ok_or(VmError::Allocation)?;
    Ok(RootEnvironment {
        realm: realm.clone(),
        table,
        session: Some(state),
    })
}
/// The live session's current primary root table, by numeric membership.
fn primary_root(state: &Rc<SharedState>, realm: &Realm) -> Option<Table> {
    let identity = {
        let contexts = state.contexts.borrow();
        let RootMembership::Object(identity) = contexts
            .first()?
            .record()?
            .membership
            .get(payload::ROOT)
            .copied()
            .unwrap_or(RootMembership::Null)
        else {
            return None;
        };
        identity
    };
    realm.heap.handle_at(realm, identity).map(Table)
}
/// Move the primary payload's root owner into one external host handle,
/// leaving the persistent root field Null exactly like the native primary
/// VM's single `_roottable` reference held by the bound runner slot.
#[expect(
    clippy::unreachable,
    reason = "The primary context of a live session always exists and owns its root table"
)]
fn take_payload_root(state: &Rc<SharedState>, realm: &Realm) -> Option<Table> {
    let primary = state.roots.borrow().as_ref()?.primary.clone()?;
    let handle = Handle::from_edge(&primary, realm);
    let released = handle.mutate(|object| match object {
        Object::Context(data) => Some(data.take_root()),
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => unreachable!("the primary context payload"),
    })?;
    drop(handle);
    let Stored::Table(edge) = released else {
        return None;
    };
    // Retain the incoming owner before releasing the displaced payload edge,
    // so the table never observes zero references (design §89).
    let table = Table(Handle::from_edge(&edge, realm));
    drop(edge);
    Some(table)
}
/// Publish the primary's current root membership without adding an owner;
/// the bound runner slot stays the root's sole owner.
#[expect(
    clippy::redundant_pub_crate,
    reason = "The crate-root root slot reads the membership publication across modules"
)]
pub(crate) fn publish_primary_root(state: &Rc<SharedState>, stored: &Stored) {
    let mut contexts = state.contexts.borrow_mut();
    let Some(record) = contexts.first_mut().and_then(ContextSlot::record_mut) else {
        return;
    };
    if let Some(membership) = record.membership.get_mut(payload::ROOT) {
        *membership = RootMembership::of(stored);
    }
}
/// A bare-local friend context (`sq_newthread` popped into a reference-only
/// local, `sqapi.cpp:82`): real friend allocation, `Init` from the passed
/// root with `sq_base_register`, then the caller's non-rooting reference.
/// The friend stays collectable; its sole owner is the caller's token.
/// # Errors
/// Returns `Allocation` when a checked construction reservation fails.
#[expect(
    clippy::redundant_pub_crate,
    reason = "The crate-root runner reads the bare-local friend path across modules"
)]
pub(crate) fn local_friend(
    state: &Rc<SharedState>,
    root: Stored,
    initial_slots: usize,
) -> Result<Edge, NativeContextError> {
    let realm = state.realm.clone();
    let friend = realm.heap.unpublished(
        &realm,
        Object::Context(Box::new(payload::ContextPayload::allocated())),
    )?;
    let Stored::Table(root) = root else {
        return Err(NativeContextError::Allocation);
    };
    let result = init_from_primary(state, &friend, initial_slots, &root);
    match result {
        Ok(_membership) => Ok(friend.publish_shared()),
        Err(failure) => {
            // Native has no tested `Init` failure oracle (design §103); this
            // rollback only guarantees zero leaked owners of the allocation.
            realm.heap.release_orphan(&friend);
            realm.heap.drain_releases();
            Err(failure)
        }
    }
}
