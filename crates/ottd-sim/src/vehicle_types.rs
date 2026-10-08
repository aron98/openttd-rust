//! Wire state for the supported vehicle callback boundary.
use serde::{Deserialize, Serialize};

/// Native company vehicle type; effects and disasters are outside this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleKind {
    /// Railway vehicle.
    Train,
    /// Road vehicle.
    Road,
    /// Ship.
    Ship,
    /// Aircraft, shadow, or rotor.
    Aircraft,
}

/// Fields consumed or changed by calendar aging and yearly accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallbackVehicle {
    /// Native pool index, including holes in the pool.
    pub id: u32,
    /// Owning human company.
    pub owner: u8,
    /// Native vehicle type.
    pub kind: VehicleKind,
    /// Native subtype; primary status is derived rather than supplied.
    pub subtype: u8,
    /// Explicit group or native `DEFAULT_GROUP` sentinel 65534.
    pub group_id: u16,
    /// Calendar age in days.
    pub age: i32,
    /// Calendar age limit in days.
    pub max_age: i32,
    /// Economy age in days.
    pub economy_age: i32,
    /// Native 16-bit reliability decrement, including wrapping shifts.
    pub reliability_spd_dec: u16,
    /// Signed profit with eight fractional bits.
    pub profit_this_year: i64,
    /// Previous year's signed profit with eight fractional bits.
    pub profit_last_year: i64,
}
impl CallbackVehicle {
    pub(crate) const fn primary(&self) -> bool {
        match self.kind {
            VehicleKind::Train | VehicleKind::Road => self.subtype & 1 != 0,
            VehicleKind::Ship => true,
            VehicleKind::Aircraft => self.subtype <= 2,
        }
    }
}

/// Profit cache belonging to a company/type/group tuple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallbackGroup {
    /// Owning human company.
    pub owner: u8,
    /// Native company vehicle type.
    pub kind: VehicleKind,
    /// Group pool index, `ALL_GROUP=65533`, or `DEFAULT_GROUP=65534`.
    pub id: u16,
    /// Sum of individually rounded display profits.
    pub profit_last_year: i64,
    /// Display profit sum for vehicles strictly older than 730 days.
    pub profit_last_year_min_age: i64,
    /// Count of vehicles strictly older than 730 days.
    pub num_vehicle_min_age: u16,
    /// Total vehicle count; callbacks preserve this cache.
    pub num_vehicle: u16,
}

/// Explicit human-company, no-news vehicle callback state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleCallbacks {
    /// Only these human company IDs may own objects; AI queues are unsupported.
    pub human_companies: Vec<u8>,
    /// Must be false: old-vehicle advice is not supported.
    pub old_vehicle_warn: bool,
    /// Must be false: income advice is not supported.
    pub vehicle_income_warn: bool,
    /// Gameplay randomizer; these callbacks preserve both words.
    pub random_state: [u32; 2],
    /// Sparse native vehicle pool in ascending ID order.
    pub vehicles: Vec<CallbackVehicle>,
    /// Group caches in arbitrary order with unique owner/type/id tuples.
    pub groups: Vec<CallbackGroup>,
}

/// One actual callback invocation, separate from whole-world tick simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum VehicleOperation {
    /// Original calendar scheduler phase, using the supplied native day fraction.
    CalendarDay {
        /// Slot modulo 74, before any subsequent tick phase.
        date_fract: u16,
    },
    /// Original economy YEAR/VEHICLE callback.
    EconomyYear,
}
