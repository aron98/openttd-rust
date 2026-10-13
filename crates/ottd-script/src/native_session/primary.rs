//! Primary context allocation, `Init` sizing and base registration.
use super::{
    base,
    context::{
        ContextId, ContextLife, ContextPhase, ContextRecord, ContextStorage, RootMembership,
    },
    error::NativeContextError,
    graph::Builder,
    payload,
    roots::TrackedTable,
};
use crate::{
    heap::{Edge, Handle, Object, Stored},
    table,
    table::scalar::Scalar,
};

/// Build the primary context in source order: allocate the collectable
/// context, publish `_root_vm`, size `Init` buffers, publish the fresh root
/// table, then register the four base closures and width slots.
pub(super) fn build(
    builder: &mut Builder,
    initial_slots: usize,
) -> Result<ContextRecord, NativeContextError> {
    let realm = builder.realm().clone();
    let life = ContextLife::new(ContextPhase::Allocated);
    // SQVM allocation: the collectable context, retained once by `_root_vm`.
    let context = realm.heap.unpublished(
        &realm,
        Object::Context(Box::new(payload::ContextPayload::allocated())),
    )?;
    let primary = context.publish_shared();
    builder.roots.primary = Some(primary);
    life.set_phase(ContextPhase::Initializing);
    let handle = Handle::from_edge(
        builder
            .roots
            .primary
            .as_ref()
            .ok_or(NativeContextError::Allocation)?,
        &realm,
    );
    let handle = &handle;
    handle.mutate(|object| match object {
        Object::Context(data) => data.init(initial_slots),
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => Ok(()),
    })?;
    let root = root_table(builder, &realm)?;
    register_base(&realm, &root, handle)?;
    let persistent = handle.read(|object| match object {
        Object::Context(data) => data.persistent().clone(),
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => std::array::from_fn(|_| Stored::null()),
    });
    life.set_phase(ContextPhase::Live);
    life.retain();
    let membership = std::array::from_fn(|index| {
        persistent
            .get(index)
            .map_or(RootMembership::Null, RootMembership::of)
    });
    Ok(ContextRecord::new(
        ContextId {
            index: 0,
            generation: 1,
        },
        life,
        ContextStorage::Primary,
        membership,
    ))
}
/// Init's fresh root table, published into the context's roottable field.
fn root_table(builder: &Builder, realm: &crate::Realm) -> Result<TrackedTable, NativeContextError> {
    let heap = realm.heap.clone();
    let root = heap.unpublished(realm, Object::Table(table::Data::new(0)?))?;
    let token = root.share();
    let edge = root.stored();
    let primary = builder
        .roots
        .primary
        .as_ref()
        .ok_or(NativeContextError::Allocation)?;
    let handle = Handle::from_edge(primary, realm);
    handle.mutate(|object| match object {
        Object::Context(data) => {
            data.publish_root(Edge::clone(&edge));
            Ok::<(), crate::VmError>(())
        }
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => Ok(()),
    })?;
    drop(handle);
    heap.drain_releases();
    Ok(TrackedTable { token })
}
/// `sq_base_register` for the primary: the fresh root table still belongs to
/// its zero-reference construction token, so registration mutates through it
/// without adding a host owner.
#[expect(
    clippy::unreachable,
    reason = "The root table token only refers to table storage"
)]
fn register_base(
    realm: &crate::Realm,
    root: &TrackedTable,
    handle: &Handle,
) -> Result<(), NativeContextError> {
    let root_edge = handle.read(|object| match object {
        Object::Context(data) => match data.persistent()[payload::ROOT] {
            Stored::Table(ref edge) => Some(Edge::clone(edge)),
            _ => None,
        },
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => None,
    });
    let Some(root_edge) = root_edge else {
        return Err(NativeContextError::Allocation);
    };
    let insert = |scalar: Scalar, hash: u64, value: Stored| {
        root.token.mutate(|object| match object {
            Object::Table(data) => data.nodes.insert(scalar, hash, value),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Context(_) => unreachable!(),
        })
    };
    base::register(
        realm,
        &|value| base::push(handle, realm, value),
        &|count| base::pop(handle, realm, count),
        &root_edge,
        &insert,
    )
}
