//! Retained-friend allocation, explicit host retention and the scoped view.
//!
//! `sq_newthread` (`sqapi.cpp:72-88`) allocates a friend context, `Init`s it
//! from the primary (`sqvm.cpp:318-337`) and pushes it onto the primary
//! stack; the retained-friend recipe then adds one explicit `RefTable`
//! retention and pops the stack slot, so the token's host retention is the
//! friend's sole owner (design §81). The token's drop performs the matching
//! release; the view leases the friend's stable payload for a borrow.
use super::{
    base,
    context::{ContextId, ContextLife, ContextPhase, ContextRecord, ContextSlot, ContextStorage},
    error::NativeContextError,
    friend_init::init_from_primary,
    payload,
    ref_identity::{OT_THREAD, RawSlot},
    session::{Phase, SharedState},
};
use crate::heap::{Edge, Handle, Object, ObjectId, Stored};
use std::rc::{Rc, Weak};

/// One explicit host retention of a friend context (design §81).
///
/// The token is the `RefTable` node's owning reference for this object. It
/// holds no runner and no implicit clone; dropping it releases exactly one
/// handle, and an explicit `retain` adds another.
#[derive(Debug)]
pub struct RetainedNativeFriend {
    pub(super) state: Weak<SharedState>,
    pub(super) generation: u64,
    pub(super) context: ContextId,
    pub(super) object: ObjectId,
    pub(super) key: RawSlot,
}
impl RetainedNativeFriend {
    /// The session-arena identity of the retained friend.
    #[cfg(test)]
    pub(super) const fn context_id(&self) -> ContextId {
        self.context
    }
    /// The friend's raw identity key, for retention witnesses.
    #[cfg(test)]
    pub(super) const fn raw_slot(&self) -> RawSlot {
        self.key
    }
}
impl Drop for RetainedNativeFriend {
    /// Release one explicit host retention; after close or retirement the
    /// drop is inert and never touches freed state (design §81).
    fn drop(&mut self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        if state.phase() != Phase::Live || state.live_generation() != self.generation {
            return;
        }
        let final_release = state
            .refs
            .borrow_mut()
            .as_mut()
            .map(|table| table.release(self.key));
        if final_release == Some(Ok(true)) {
            retire_record(&state, self.context);
        }
        state.realm.heap.drain_releases();
    }
}
/// A short-lived handle on the primary context, like the construction-time
/// stack access of the source factory.
pub(super) fn primary_handle(state: &Rc<SharedState>) -> Result<Handle, NativeContextError> {
    let primary = state
        .roots
        .borrow()
        .as_ref()
        .and_then(|roots| roots.primary.clone())
        .ok_or(NativeContextError::Allocation)?;
    Ok(Handle::from_edge(&primary, &state.realm))
}
/// `sq_newthread` plus the retained-friend recipe: reserve the registry
/// slot, allocate the friend, `Init` it from the primary, `Push` onto the
/// primary stack, `AddRef` into the physical `RefTable`, then `Pop`.
pub(super) fn retain_friend(
    state: &Rc<SharedState>,
    initial_slots: usize,
) -> Result<RetainedNativeFriend, NativeContextError> {
    if state.phase() != Phase::Live {
        return Err(NativeContextError::ClosedContext);
    }
    if initial_slots == 0 {
        return Err(NativeContextError::InvalidInitialSlots);
    }
    let realm = state.realm.clone();
    let (slot_index, id) = reserve_slot(state)?;
    let friend = realm.heap.unpublished(
        &realm,
        Object::Context(Box::new(payload::ContextPayload::allocated())),
    )?;
    // The retained friend's `Init` copies the primary's `_roottable` owner
    // (`sqvm.cpp:330`), so the shared root table reference comes out of the
    // primary payload before the friend allocation begins.
    #[expect(
        clippy::unreachable,
        reason = "The live session's primary context owns its root table"
    )]
    let root = primary_handle(state)?.read(|object| match object {
        Object::Context(data) => match data.persistent()[payload::ROOT] {
            Stored::Table(ref root) => Edge::clone(root),
            Stored::Scalar(_)
            | Stored::Array(_)
            | Stored::NativeClosure(_)
            | Stored::ScriptClosure(_)
            | Stored::Context(_) => unreachable!("the primary root table"),
        },
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => unreachable!("the primary context payload"),
    });
    let membership = match init_from_primary(state, &friend, initial_slots, &root) {
        Ok(membership) => membership,
        Err(failure) => {
            // Native has no tested `Init` failure oracle (design §103); this
            // rollback only guarantees zero leaked owners of the allocation.
            realm.heap.release_orphan(&friend);
            realm.heap.drain_releases();
            return Err(failure);
        }
    };
    // `friendvm->Push(v)` then `sq_addref` then `sq_pop` (`sqapi.cpp:82`):
    // the stack slot owns the friend until the RefTable node takes over.
    let owner = friend.publish_shared();
    let object = owner.identity();
    let retained = Edge::clone(&owner);
    let key = RawSlot::of_object(object, OT_THREAD, &realm)?;
    let primary = primary_handle(state)?;
    base::push(&primary, &realm, Stored::Context(owner))?;
    state.retain_stored(Stored::Context(retained))?;
    base::pop(&primary, &realm, 1);
    let life = ContextLife::new(ContextPhase::Live);
    life.retain();
    let record = ContextRecord::new(id, life, ContextStorage::Friend, membership);
    state
        .contexts
        .borrow_mut()
        .get_mut(slot_index)
        .ok_or(NativeContextError::Allocation)?
        .publish(record);
    Ok(RetainedNativeFriend {
        state: Rc::downgrade(state),
        generation: state.live_generation(),
        context: id,
        object,
        key,
    })
}
/// Reserve one registry slot identity: reuse increments the generation of a
/// retired slot without wrapping, a fresh slot starts at generation one, and
/// an exhausted or missing reservation fails the checked allocation.
fn reserve_slot(state: &Rc<SharedState>) -> Result<(usize, ContextId), NativeContextError> {
    let mut contexts = state.contexts.borrow_mut();
    if let Some(index) = contexts.iter().position(|slot| slot.record().is_none()) {
        let Some(id) = contexts.get(index).and_then(|slot| slot.reserve(index)) else {
            return Err(NativeContextError::Allocation);
        };
        return Ok((index, id));
    }
    contexts
        .try_reserve(1)
        .map_err(|_| NativeContextError::Allocation)?;
    let index = contexts.len();
    contexts.push(ContextSlot::vacant());
    let Some(id) = contexts.get(index).and_then(|slot| slot.reserve(index)) else {
        return Err(NativeContextError::Allocation);
    };
    Ok((index, id))
}
/// Repeat one explicit host retention: the node's handle count grows by one
/// and the returned token drops one release.
pub(super) fn retain_again(
    state: &Rc<SharedState>,
    token: &RetainedNativeFriend,
) -> Result<RetainedNativeFriend, NativeContextError> {
    let cloned = {
        let mut refs = state.refs.borrow_mut();
        refs.as_mut()
            .and_then(|table| table.stored_clone(token.key))
    };
    let Some(cloned) = cloned else {
        return Err(NativeContextError::StaleContext);
    };
    state.retain_stored(cloned)?;
    Ok(RetainedNativeFriend {
        state: Rc::downgrade(state),
        generation: state.live_generation(),
        context: token.context,
        object: token.object,
        key: token.key,
    })
}
/// Borrow one retained friend for a scoped view on both the session and the
/// token: validate the token, then move the friend's stable private payload
/// allocation into the lease (design §81).
pub(super) fn friend<'a>(
    state: &'a Rc<SharedState>,
    token: &'a RetainedNativeFriend,
) -> Result<super::friend_view::NativeFriendView<'a>, NativeContextError> {
    let Some(token_state) = token.state.upgrade() else {
        return Err(NativeContextError::ForeignContext);
    };
    if !Rc::ptr_eq(&token_state, state) {
        return Err(NativeContextError::ForeignContext);
    }
    if state.phase() != Phase::Live {
        return Err(NativeContextError::ClosedContext);
    }
    if state.live_generation() != token.generation {
        return Err(NativeContextError::StaleContext);
    }
    let live = state
        .contexts
        .borrow()
        .get(token.context.index)
        .and_then(|slot| slot.record())
        .is_some_and(|record| {
            record.id == token.context
                && record.storage == ContextStorage::Friend
                && record.life.phase() == ContextPhase::Live
        });
    if !live {
        return Err(NativeContextError::StaleContext);
    }
    super::friend_view::lease(state, token)
}
/// Retire one friend's registry record after its final release; the slot
/// becomes reusable with an incremented identity generation.
fn retire_record(state: &Rc<SharedState>, context: ContextId) {
    let mut contexts = state.contexts.borrow_mut();
    let Some(slot) = contexts.get_mut(context.index) else {
        return;
    };
    let Some(record) = slot.retire() else {
        return;
    };
    if record.id != context {
        slot.publish(record);
        return;
    }
    record.life.release();
    record.life.set_phase(ContextPhase::Retired);
}
