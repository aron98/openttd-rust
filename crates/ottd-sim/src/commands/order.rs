use serde::{Deserialize, Serialize};

/// Native serialized order payload accepted by `CMD_INSERT_ORDER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsertOrder {
    /// Native order type with non-stop and stop-location bits.
    #[serde(rename = "type")]
    pub kind: u8,
    /// Type-dependent native load, unload or depot flags.
    pub flags: u8,
    /// Destination station or depot identity.
    pub dest: u16,
    /// Cargo refit selector; insertion requires native `CARGO_NO_REFIT` (254).
    pub refit_cargo: u8,
    /// Timetable wait; insertion requires zero.
    pub wait_time: u16,
    /// Timetable travel; insertion requires zero.
    pub travel_time: u16,
    /// Timetable maximum speed; insertion requires 65535.
    pub max_speed: u16,
}
