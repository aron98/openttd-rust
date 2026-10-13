//! The shared `sq_base_register` recipe for one context's root table
//! (`sqbaselib.cpp:286-313`): push the roottable, create each base closure in
//! source order into a root slot, write the version/width slots, then pop
//! the root. Existing keys are overwritten like native `SQTable::NewSlot`
//! (`sqtable.cpp:99-106`), which releases the displaced closure owner.
use super::error::NativeContextError;
use crate::{
    Realm,
    heap::{Handle, Stored},
    native_closure::{NativeClosure, NativeRegistry},
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
/// `sq_base_register`: push the context's root table, register the four base
/// closures and the width slots through real stack writes, then pop the
/// root. The `push`/`pop` callbacks own the context's physical stack access
/// and `insert` the root-table write path of the caller.
/// # Errors
/// Returns `Allocation` when a construction reservation fails.
pub(super) fn register(
    realm: &Realm,
    push: &dyn Fn(Stored) -> Result<(), NativeContextError>,
    pop: &dyn Fn(i64),
    root: &crate::heap::Edge,
    insert: &dyn Fn(Scalar, u64, Stored) -> Result<(), crate::VmError>,
) -> Result<(), NativeContextError> {
    push(Stored::Table(crate::heap::Edge::clone(root)))?;
    for row in NativeRegistry::Base.registrations() {
        let closure = NativeClosure::unpublished(realm, row)?;
        let name = realm.string(row.name());
        let owner = closure.stored();
        let value = Stored::NativeClosure(crate::heap::Edge::clone(&owner));
        push(Stored::Scalar(Scalar::String(name.clone())))?;
        push(Stored::NativeClosure(owner))?;
        let hash = Scalar::String(name.clone()).hash()?;
        insert(Scalar::String(name), hash, value)?;
        pop(2);
    }
    for (name, entry) in BASE_DATA {
        let scalar = Scalar::String(realm.string(name));
        let hash = scalar.hash()?;
        let value = match entry {
            BaseData::Version => Stored::Scalar(Scalar::String(realm.string(BaseData::VERSION))),
            BaseData::Integer(value) => Stored::Scalar(Scalar::Integer(i64::from(value))),
        };
        push(Stored::Scalar(scalar.clone()))?;
        push(value.clone())?;
        insert(scalar, hash, value)?;
        pop(2);
    }
    pop(1);
    Ok(())
}
/// `SQVM::Push` through a live context handle: retain the value into the
/// next stack slot.
/// # Errors
/// Returns `Allocation` when the doubled stack cannot be reserved.
pub(super) fn push(
    handle: &Handle,
    realm: &Realm,
    value: Stored,
) -> Result<(), NativeContextError> {
    handle.mutate(|object| match object {
        crate::heap::Object::Context(data) => data.push(value),
        crate::heap::Object::Array(_)
        | crate::heap::Object::NativeClosure(_)
        | crate::heap::Object::ScriptClosure(_)
        | crate::heap::Object::Table(_) => Ok(()),
    })?;
    realm.heap.drain_releases();
    Ok(())
}
/// `SQVM::Pop` through a live context handle: clear the topmost slots and
/// release their owners immediately.
pub(super) fn pop(handle: &Handle, realm: &Realm, count: i64) {
    let released = handle.mutate(|object| match object {
        crate::heap::Object::Context(data) => data.pop(count),
        crate::heap::Object::Array(_)
        | crate::heap::Object::NativeClosure(_)
        | crate::heap::Object::ScriptClosure(_)
        | crate::heap::Object::Table(_) => Vec::new(),
    });
    for piece in released {
        drop(piece);
        realm.heap.drain_releases();
    }
}
