use super::super::RailVehicleCache;
use crate::runtime::VehicleId;
use serde::Serialize;

/// Domain refusals for the isolated complete-consist transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConsistRefusal {
    /// Only a nonempty train roster is supported.
    #[error("requires train records")]
    Roster,
    /// Exactly one rooted chain is supported.
    #[error("requires one complete chain")]
    MultipleRoots,
    /// Links must form one complete, nonoverlapping, acyclic chain.
    #[error("invalid consist graph")]
    Graph,
    /// Articulated/free/malformed subtypes need other restoration paths.
    #[error("unsupported train subtype")]
    Subtype,
    /// Native multihead repair and unmatched units are outside this boundary.
    #[error("unsupported multihead pairing")]
    Multihead,
    /// Original steam/diesel/wagon properties only.
    #[error("unsupported rail specification")]
    Engine,
    /// Orders and shared or backup order context are unsupported.
    #[error("unsupported rail orders")]
    Orders,
    /// Every unit must have valid empty cargo.
    #[error("unsupported rail cargo")]
    Cargo,
    /// The root must be stopped; no part may move, hide or crash.
    #[error("unsupported rail motion")]
    Motion,
    /// All parts and their track must belong to the root's valid owner.
    #[error("inconsistent rail ownership")]
    Owner,
    /// Only compatible plain normal rail is supported.
    #[error("unsupported rail track")]
    Track,
    /// Parts must form an aligned, correctly spaced flat consist.
    #[error("unsupported consist geometry")]
    Geometry,
    /// Saved uphill/downhill flags need further getter semantics.
    #[error("unsupported saved slope flags")]
    SlopeFlags,
    /// Native signed capacity accumulation must be representable.
    #[error("consist exceeds native arithmetic domain")]
    Limits,
}

/// Failure before atomic publication of consist saved edits and caches.
#[derive(Debug, thiserror::Error)]
pub enum ConsistError {
    /// An unsupported domain predicate.
    #[error(transparent)]
    Unsupported(#[from] ConsistRefusal),
    /// A part failed admission after earlier records were staged privately.
    #[error("part {id:?}: {reason} ({staged_parts} privately staged records)")]
    Part {
        /// Rejected train identity.
        id: VehicleId,
        /// Unsupported predicate.
        reason: ConsistRefusal,
        /// Actual candidate record count; none of these edits were published.
        staged_parts: usize,
    },
    /// A required saved field has the wrong representation or width.
    #[error("invalid consist field {0:?}")]
    Field(super::fields::ConsistField),
    /// Content configuration is not a valid vanilla catalog.
    #[error(transparent)]
    Content(#[from] crate::content::ContentError),
    /// Saved transaction validation failed without publication.
    #[error(transparent)]
    World(#[from] ottd_save::world::WorldError),
}

/// Complete after-load cache and reconstructed links of one train part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsistPart {
    /// Actual per-part caches; nonfront aggregate caches retain constructor defaults.
    #[serde(flatten)]
    pub cache: RailVehicleCache,
    /// Chain root.
    pub first: VehicleId,
    /// Previous part in chain order, independently of pool order.
    pub previous: Option<VehicleId>,
    /// Next part.
    pub next: Option<VehicleId>,
    /// Paired original multihead part.
    pub other_multiheaded_part: Option<VehicleId>,
    /// Front-only stopped `GetAcceleration` result, derived rather than cached or saved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub braking_acceleration: Option<i32>,
}

/// Restored parts in native vehicle-pool order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsistCaches {
    /// Single front engine identity.
    pub front: VehicleId,
    /// All train parts, including original multihead rear units.
    pub parts: Vec<ConsistPart>,
}
