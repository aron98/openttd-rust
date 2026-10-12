//! The retained-friend `Init` recipe (`sqvm.cpp:318-337`, `sqbaselib.cpp`).
use super::{
    base, context::RootMembership, error::NativeContextError, friend::primary_handle, payload,
    session::SharedState,
};
use crate::{
    heap::{Edge, Handle, Object, Stored, Unpublished},
    table::scalar::Scalar,
};
use std::rc::Rc;

/// `SQVM::Init` friend path (`sqvm.cpp:318-337`): size the physical stack
/// and four call records, copy the primary's roottable, errorhandler and
/// debughook fields, then run `sq_base_register` against the friend's own
/// stack; the shared root table's existing keys are overwritten and their
/// displaced closure owners released (`sqtable.cpp:99-106`). The registry
/// memberships of the friend's five persistent fields come back for its
/// record.
#[expect(
    clippy::unreachable,
    reason = "The primary context and root table of a live session always exist"
)]
pub(super) fn init_from_primary(
    state: &Rc<SharedState>,
    friend: &Unpublished,
    initial_slots: usize,
) -> Result<[RootMembership; 5], NativeContextError> {
    let realm = &state.realm;
    // _stack.resize / four call records (sqvm.cpp:320-326).
    friend
        .mutate(|object| match object {
            Object::Context(data) => data.init(initial_slots),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Table(_) => Ok(()),
        })
        .map_err(NativeContextError::from)?;
    // Field copies (sqvm.cpp:329-333).
    let primary = primary_handle(state)?;
    let (root_edge, error, handler, debug) = primary.read(|object| match object {
        Object::Context(data) => {
            let fields = data.persistent();
            let root = match &fields[payload::ROOT] {
                Stored::Table(edge) => Edge::clone(edge),
                _ => unreachable!("the primary root table"),
            };
            (
                root,
                fields[payload::ERROR].clone(),
                fields[payload::HANDLER].clone(),
                fields[payload::DEBUG].clone(),
            )
        }
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => unreachable!("the primary root table"),
    });
    drop(primary);
    friend.mutate(|object| match object {
        Object::Context(data) => {
            data.publish_friend_fields(root_edge.clone(), error, handler, debug);
        }
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => {}
    });
    // sq_base_register(this) (sqvm.cpp:335): the friend's stack carries the
    // root reference, and the table is mutated through that reference, like
    // native `CreateSlot` walking the pushed stack slot.
    let access = Handle {
        edge: Edge::internal(&root_edge),
        realm: realm.clone(),
    };
    let insert = |scalar: Scalar, hash: u64, value: Stored| {
        access.mutate(|object| match object {
            Object::Table(data) => data.nodes.insert(scalar, hash, value),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Context(_) => unreachable!(),
        })
    };
    let push = |value: Stored| {
        friend
            .mutate(|object| match object {
                Object::Context(data) => data.push(value),
                Object::Array(_)
                | Object::NativeClosure(_)
                | Object::ScriptClosure(_)
                | Object::Table(_) => Ok(()),
            })
            .map_err(NativeContextError::from)
    };
    let pop = |count: i64| {
        let released = friend.mutate(|object| match object {
            Object::Context(data) => data.pop(count),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Table(_) => Vec::new(),
        });
        for piece in released {
            drop(piece);
        }
    };
    base::register(realm, &push, &pop, &root_edge, &insert)?;
    let mut membership = [RootMembership::Null; 5];
    friend.mutate(|object| {
        if let Object::Context(data) = object {
            for (target, slot) in membership.iter_mut().zip(data.persistent()) {
                *target = RootMembership::of(slot);
            }
        }
    });
    Ok(membership)
}
