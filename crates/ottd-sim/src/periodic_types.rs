//! State consumed by house, company, and station bookkeeping callbacks.
use crate::Map;
use serde::{Deserialize, Serialize};

/// Whole raw map consumed by the yearly house-age scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseCallbacks {
    /// Every raw tile; all fields other than completed-house m5 are preserved.
    pub map: Map,
    /// Gameplay random state, unchanged by the callback.
    pub random_state: [u32; 2],
}

/// The company's entire three-year expense table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanyExpenses {
    /// Native company pool ID, below 15.
    pub id: u8,
    /// Current year first, followed by two prior years; 13 expense categories.
    pub yearly_expenses: [[i64; 13]; 3],
}

/// Company yearly bookkeeping without interactive financial UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanyCallbacks {
    /// Must be false; financial windows and year-end sounds are unsupported.
    pub show_finances: bool,
    /// Gameplay random state, unchanged by the callback.
    pub random_state: [u32; 2],
    /// Complete modeled company pool, sorted by unique native ID.
    pub companies: Vec<CompanyExpenses>,
}

/// Station cargo status and adjacent rating scalars retained across rollover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationCargo {
    /// All eight raw status bits.
    pub status: u8,
    /// Rating intervals since pickup, preserved.
    pub time_since_pickup: u8,
    /// Cargo rating, preserved.
    pub rating: u8,
    /// Last loading vehicle speed, preserved.
    pub last_speed: u8,
    /// Last loading vehicle age, preserved.
    pub last_age: u8,
    /// Fractional waiting cargo amount, preserved.
    pub amount_fract: u8,
}

/// Native station entry; waypoints are not station callback objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationStatus {
    /// Native station pool ID, below 64000.
    pub id: u16,
    /// Exactly 64 cargo slots, including unavailable cargo types.
    pub goods: Vec<StationCargo>,
}

/// Entire modeled station-status pool for the monthly callback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationCallbacks {
    /// Gameplay random state, unchanged by the callback.
    pub random_state: [u32; 2],
    /// Stations sorted by unique pool ID.
    pub stations: Vec<StationStatus>,
}
