use serde::{Deserialize, Serialize};

/// Native `CMD_BUILD_ROAD_STOP` payload; validation preserves native phase order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRoadStop {
    /// Number of tiles along X.
    pub width: u8,
    /// Number of tiles along Y.
    pub length: u8,
    /// Native bus (0) or truck (1) stop type.
    pub stop_type: u8,
    /// Build a drive-through rather than terminal stop.
    pub drive_through: bool,
    /// Native entrance diagonal direction.
    pub direction: u8,
    /// Native road type.
    pub road_type: u8,
    /// Native road-stop specification class.
    pub spec_class: u16,
    /// Specification index within its class.
    pub spec_index: u16,
    /// Station ID, new station (65533), or automatic selection (65535).
    pub station_to_join: u16,
    /// Allow a separate adjacent station.
    pub adjacent: bool,
}
