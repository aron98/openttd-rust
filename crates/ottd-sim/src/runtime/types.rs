//! Public runtime identity and restoration error vocabulary.
use super::pools;
use crate::content::ContentError;
use serde::Serialize;

/// Native sparse vehicle pool identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct VehicleId(pub(super) u32);
impl VehicleId {
    /// Wrap a pool index; lookups check whether it exists.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    /// Native pool index, not a saved pointer encoding.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Rejected or malformed runtime restoration input.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Invalid native vehicle allocation metadata.
    #[error(transparent)]
    Pool(#[from] pools::PoolError),
    /// Saved order backups require an explicit host load context.
    #[error("loaded order backups require explicit runtime context")]
    OrderContextRequired,
    /// Pinned native deletion would assert because a loaded object lost its pool index.
    #[error("native backup deletion invariant: physical slot {slot} has object index {index}")]
    NativeBackupIndex {
        /// Authoritative physical pool slot / saved row ID.
        slot: u32,
        /// Original runtime object's index after constructor/load.
        index: u8,
    },
    /// Invalid saved transaction.
    #[error(transparent)]
    World(#[from] ottd_save::world::WorldError),
    /// Unsupported content configuration.
    #[error(transparent)]
    Content(#[from] ContentError),
    /// Feature requiring a later runtime family.
    #[error("unsupported runtime: {0}")]
    Unsupported(&'static str),
    /// Missing or invalid native saved field.
    #[error("invalid runtime input: {0}")]
    Invalid(&'static str),
}
