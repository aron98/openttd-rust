//! Stable raw identity and native hash/equality keys for `RefTable` slots.
//!
//! The design's identity prerequisite (§121-123): every native object owns
//! one stable actual-payload allocation whose address is the raw value an
//! `SQObjectPtr` carries for refcounted pointer types — unique per live
//! object, identical for aliases, ended by actual retirement, with address
//! reuse after retirement permitted exactly like the native allocator.
//! `HashObj` cannot silently substitute the arena identity: strings hash by
//! their interned `_hash`, pointer types by the payload address
//! arithmetic-shifted three, and equality compares both the raw value and
//! the exact native type tag (`sqtable.h:12-21`, `sqtable.h:13`).
use super::error::NativeContextError;
use crate::{
    Realm,
    heap::{ObjectId, Stored},
    table::scalar::Scalar,
};

/// Exact native `SQObjectType` raw values of the retained slot kinds
/// (`squirrel.h:95-130`: `_RT_*` member bits, `SQOBJECT_REF_COUNTED`
/// 0x08000000, `SQOBJECT_DELEGABLE` 0x02000000).
pub(super) const OT_STRING: u32 = 0x0800_0010;
pub(super) const OT_TABLE: u32 = 0x0A00_0020;
pub(super) const OT_ARRAY: u32 = 0x0800_0040;
pub(super) const OT_CLOSURE: u32 = 0x0800_0100;
pub(super) const OT_NATIVECLOSURE: u32 = 0x0800_0200;
pub(super) const OT_THREAD: u32 = 0x0800_1000;

/// One slot's raw identity: the payload address (or interned string
/// allocation address) plus the exact native type tag and the `HashObj`
/// result the source compares and buckets with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RawSlot {
    raw: u64,
    tag: u32,
    hash: u64,
}
impl RawSlot {
    /// Direct raw-value key construction; test-only bucket-mechanics probes
    /// use this because real payload addresses cannot force deterministic
    /// hash collisions (`sqstate.cpp:467-487 Get`).
    #[cfg(test)]
    pub(super) const fn new(raw: u64, tag: u32) -> Self {
        Self {
            raw,
            tag,
            hash: raw >> 3,
        }
    }
    /// `HashObj` main-position hash of this key: pointer keys use the
    /// address shift (`sqtable.h:13 hashptr`), strings their native
    /// interned string hash.
    pub(super) const fn hash(&self) -> u64 {
        self.hash
    }
    /// The raw payload address a native `SQObjectPtr` would compare.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "identity witnesses; the atom's tests read it")
    )]
    pub(super) const fn raw(&self) -> u64 {
        self.raw
    }
    /// The exact native type tag; source equality needs raw value AND type
    /// (`sqstate.cpp:473-474`).
    pub(super) const fn tag(&self) -> u32 {
        self.tag
    }
    /// Raw identity of one interned string scalar: its allocation address
    /// is unique per live interning like an `SQString` of the shared string
    /// table, and the hash is the native `_hash` of the interned bytes.
    pub(super) fn of_string(value: &Scalar) -> Result<Self, NativeContextError> {
        let Scalar::String(bytes) = value else {
            return Err(NativeContextError::Allocation);
        };
        let raw = u64::try_from(std::ptr::from_ref(bytes.as_bytes()).addr())
            .map_err(|_| NativeContextError::Allocation)?;
        Ok(Self {
            raw,
            tag: OT_STRING,
            hash: value.hash().map_err(NativeContextError::from)?,
        })
    }
    /// Raw identity of one collectable object: the stable actual-payload
    /// address of its live heap entry.
    pub(super) fn of_object(
        id: ObjectId,
        tag: u32,
        realm: &Realm,
    ) -> Result<Self, NativeContextError> {
        realm
            .heap
            .payload_address(id)
            .map(|raw| Self {
                raw,
                tag,
                hash: raw >> 3,
            })
            .ok_or(NativeContextError::Allocation)
    }
    /// Compute the raw identity of one stored value, or `None` when the
    /// value carries no refcounted object for `sq_addref` to retain
    /// (`sqapi.cpp:155-158` ignores non-refcounted inputs).
    pub(super) fn of_stored(
        value: &Stored,
        realm: &Realm,
    ) -> Option<Result<Self, NativeContextError>> {
        match value {
            Stored::Scalar(scalar @ Scalar::String(_)) => Some(Self::of_string(scalar)),
            Stored::Scalar(_) => None,
            Stored::Table(edge) => Some(Self::of_object(edge.identity(), OT_TABLE, realm)),
            Stored::Array(edge) => Some(Self::of_object(edge.identity(), OT_ARRAY, realm)),
            Stored::ScriptClosure(edge) => {
                Some(Self::of_object(edge.identity(), OT_CLOSURE, realm))
            }
            Stored::NativeClosure(edge) => {
                Some(Self::of_object(edge.identity(), OT_NATIVECLOSURE, realm))
            }
            Stored::Context(edge) => Some(Self::of_object(edge.identity(), OT_THREAD, realm)),
        }
    }
}
