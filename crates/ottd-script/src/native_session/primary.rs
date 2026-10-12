//! Primary context allocation, `Init` sizing and base registration.
use super::{
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
    native_closure::{NativeClosure, NativeRegistry},
    table,
    table::scalar::Scalar,
};

/// Root-table data entries after the four base closures, in source order.
const BASE_DATA: [(&[u8], BaseData); 4] = [
    (b"_version_", BaseData::Version),
    (b"_charsize_", BaseData::Integer(1)),
    (b"_intsize_", BaseData::Integer(8)),
    (b"_floatsize_", BaseData::Integer(4)),
];
enum BaseData {
    Version,
    Integer(u32),
}
impl BaseData {
    const VERSION: &'static [u8] = b"Squirrel 2.2.5 stable - With custom OpenTTD modifications";
}
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
/// `sq_base_register`: push the root, create each base closure in source
/// order into a root slot, write the version/width slots, then pop the root.
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
    push(handle, realm, Stored::Table(root_edge))?;
    for row in NativeRegistry::Base.registrations() {
        let closure = NativeClosure::unpublished(realm, row)?;
        let name = realm.string(row.name());
        let owner = closure.stored();
        let value = Stored::NativeClosure(Edge::clone(&owner));
        push(handle, realm, Stored::Scalar(Scalar::String(name.clone())))?;
        push(handle, realm, Stored::NativeClosure(owner))?;
        let scalar = Scalar::String(name);
        let hash = scalar.hash()?;
        root.token.mutate(|object| match object {
            Object::Table(data) => data.nodes.insert(scalar, hash, value),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Context(_) => unreachable!(),
        })?;
        pop(handle, realm, 2);
    }
    for (name, entry) in BASE_DATA {
        let scalar = Scalar::String(realm.string(name));
        let hash = scalar.hash()?;
        let value = match entry {
            BaseData::Version => Stored::Scalar(Scalar::String(realm.string(BaseData::VERSION))),
            BaseData::Integer(value) => Stored::Scalar(Scalar::Integer(i64::from(value))),
        };
        push(handle, realm, Stored::Scalar(scalar.clone()))?;
        push(handle, realm, value.clone())?;
        root.token.mutate(|object| match object {
            Object::Table(data) => data.nodes.insert(scalar, hash, value),
            Object::Array(_)
            | Object::NativeClosure(_)
            | Object::ScriptClosure(_)
            | Object::Context(_) => unreachable!(),
        })?;
        pop(handle, realm, 2);
    }
    pop(handle, realm, 1);
    Ok(())
}
/// `SQVM::Push`: retain the value into the next stack slot.
fn push(handle: &Handle, realm: &crate::Realm, value: Stored) -> Result<(), NativeContextError> {
    handle.mutate(|object| match object {
        Object::Context(data) => data.push(value),
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => Ok(()),
    })?;
    realm.heap.drain_releases();
    Ok(())
}
/// `SQVM::Pop`: clear the topmost slots and release their owners immediately.
fn pop(handle: &Handle, realm: &crate::Realm, count: i64) {
    let released = handle.mutate(|object| match object {
        Object::Context(data) => data.pop(count),
        Object::Array(_)
        | Object::NativeClosure(_)
        | Object::ScriptClosure(_)
        | Object::Table(_) => Vec::new(),
    });
    for piece in released {
        drop(piece);
        realm.heap.drain_releases();
    }
}
