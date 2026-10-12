//! The exact source close sequence of `sq_close` plus `~SQSharedState`.
use super::{
    context::{ContextPhase, ContextSlot, ContextStorage},
    session::{Phase, SharedState},
};
use crate::heap::{Handle, Object};

/// Run the full close sequence once; later calls are inert. Marking `Closing`
/// first invalidates every weak capability before any owner is released.
pub(super) fn close(state: &SharedState) {
    if state.phase() != Phase::Live {
        return;
    }
    state.phase.set(Phase::Closing);
    let realm = state.realm.clone();
    let mut roots = state.roots.borrow_mut().take();
    let mut refs = state.refs.borrow_mut().take();
    let mut contexts = state.contexts.borrow_mut();
    let record = contexts.iter_mut().find_map(|slot: &mut ContextSlot| {
        if slot
            .record()
            .is_some_and(|record| record.storage == ContextStorage::Primary)
        {
            slot.retire()
        } else {
            None
        }
    });
    drop(contexts);
    let Some(roots) = roots.as_mut() else {
        state.phase.set(Phase::Closed);
        return;
    };
    // sq_close: first primary Finalize clears root, error, handler, debug,
    // temp, the call storage and the complete stack in reverse.
    finalize_primary(&realm, roots);
    if let Some(record) = &record {
        record.life.set_phase(ContextPhase::FinalizedReferenceLive);
    }
    // Clear the constructor index.
    drop(roots.constructor.take());
    // Finalize registry, consts and the metamethod map, then release each.
    for (edge, finalize) in [
        (roots.registry.take(), true),
        (roots.consts.take(), true),
        (roots.meta.take(), true),
    ] {
        if let Some(edge) = edge {
            if finalize {
                realm.heap.finalize_table_edge(&edge);
            }
            drop(edge);
        }
    }
    // Clear the system strings in reverse.
    while let Some(string) = roots.system_strings.pop() {
        drop(string);
    }
    // The second primary Finalize stays valid on cleared fields.
    finalize_primary(&realm, roots);
    // Drop the shared primary owner; the context storage retires with it.
    if let Some(primary) = roots.primary.take() {
        drop(primary);
        if let Some(record) = &record {
            if record.life.release() {
                record.life.set_phase(ContextPhase::Retired);
            }
        }
    }
    // Release the default tables in declaration order.
    for (_, edge) in std::mem::take(&mut roots.defaults) {
        drop(edge);
    }
    // Finalize the physical RefTable ascending, releasing each stored owner
    // after the table borrow ends, then release its storage; every retained
    // friend's sole owner is gone with its node.
    if let Some(table) = refs.as_mut() {
        let released = table.finalize();
        drop(refs);
        for piece in released {
            drop(piece);
            realm.heap.drain_releases();
        }
    } else {
        drop(refs);
    }
    retire_friend_records(state);
    // Sweep and drain the remaining collectables; every shared owner is gone.
    realm.heap.drain_releases();
    // Destroy the type, system and metamethod vectors last; the string table
    // dies with the realm's own storage.
    roots.metamethods.clear();
    roots.types.clear();
    state.phase.set(Phase::Closed);
}
/// Retire every remaining friend record: the `RefTable` close above released
/// each friend's sole owning reference, so every friend context is retired
/// storage whose registry slot is reusable with an incremented generation.
fn retire_friend_records(state: &SharedState) {
    let mut contexts = state.contexts.borrow_mut();
    for slot in contexts.iter_mut() {
        let Some(record) = slot.retire() else {
            continue;
        };
        if record.storage == ContextStorage::Friend {
            record.life.release();
            record.life.set_phase(ContextPhase::Retired);
        } else {
            slot.publish(record);
        }
    }
}
/// `SQVM::Finalize` through a short borrowed handle, releasing each taken
/// payload piece immediately after the borrow ends.
fn finalize_primary(realm: &crate::Realm, roots: &super::roots::SharedRoots) {
    let Some(primary) = roots.primary.as_ref() else {
        return;
    };
    let handle = Handle::from_edge(primary, realm);
    let released = handle.mutate(|object| match object {
        Object::Context(payload) => payload.finalize(),
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => Vec::new(),
    });
    drop(handle);
    for piece in released {
        drop(piece);
        realm.heap.drain_releases();
    }
}
