use super::{RailRestoreError, record::RailInput};
use crate::runtime::VehicleId;
use serde::Serialize;

/// Actual train/ground/vehicle caches rebuilt by the bounded after-load transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RailVehicleCache {
    /// Sparse native vehicle identity.
    pub id: VehicleId,
    /// Engine rail-type bit mask.
    pub railtypes: u64,
    /// Union of powered rail types.
    pub compatible_railtypes: u64,
    /// Front engine's predecessor engine identity (native invalid sentinel).
    pub first_engine: u16,
    /// Part length in eighths of a standard vehicle.
    pub vehicle_length: u8,
    /// Complete consist length in the same units.
    pub total_length: u16,
    /// Internal maximum speed.
    pub max_speed: u16,
    /// Ticks between cargo-aging events.
    pub cargo_age_period: u16,
    /// Resolved visual-effect bit field.
    pub visual_effect: u8,
    /// Total consist weight in tonnes.
    pub weight: u32,
    /// Part resistance on slopes, in newtons.
    pub slope_resistance: u32,
    /// Native narrowed axle resistance.
    pub axle_resistance: u16,
    /// Total power in horsepower.
    pub power: u32,
    /// Maximum tractive effort in newtons.
    pub max_tractive_effort: u32,
    /// Rail-type-limited internal speed.
    pub max_track_speed: u16,
    /// Native drag coefficient.
    pub air_drag: u32,
    /// Load-initialized display-speed cache.
    pub last_speed: u16,
    /// Load-initialized cargo occupancy percentage.
    pub trip_occupancy: i8,
    /// Rail property 0x25.
    pub user_def_data: u8,
    /// Whether all consist parts can tilt.
    pub tilt: bool,
    /// Fixed-point curve-speed modifier, with eight fractional bits.
    pub curve_speed_mod: i16,
    /// Curve-limited speed.
    pub max_curve_speed: u16,
    /// Whether a wagon sprite override exists.
    pub override_present: bool,
    /// Load-reset native colour mapping.
    pub colourmap: u32,
}

pub(super) fn restore(input: &RailInput) -> Result<RailVehicleCache, RailRestoreError> {
    // train_cmd.cpp ConsistChanged(CCF_SAVELOAD), then ground_vehicle.cpp CargoChanged/PowerChanged.
    let spec = input.spec;
    let length = 8_u8.saturating_sub(spec.shorten_factor.min(7));
    let max_speed = if spec.max_speed == 0 {
        u16::MAX
    } else {
        spec.max_speed
    };
    let drag = match spec.air_drag {
        0 if max_speed <= 10 => 192,
        0 => 2048_u16
            .checked_div(max_speed)
            .ok_or(RailRestoreError::Invalid("max_speed"))?
            .max(1),
        1 => 0,
        value => u16::from(value),
    };
    let weight = u32::from(spec.weight);
    Ok(RailVehicleCache {
        id: input.id,
        railtypes: spec.railtypes,
        compatible_railtypes: 3,
        first_engine: u16::MAX,
        vehicle_length: length,
        total_length: u16::from(length),
        max_speed,
        cargo_age_period: input.info.cargo_age_period,
        visual_effect: if spec.engclass == 0 { 0x14 } else { 0x28 },
        weight: weight.max(1),
        slope_resistance: weight
            .wrapping_mul(u32::from(input.slope))
            .wrapping_mul(100),
        axle_resistance: spec.weight.wrapping_mul(10),
        power: u32::from(spec.power),
        max_tractive_effort: weight
            .wrapping_mul(u32::from(spec.tractive_effort))
            .wrapping_mul(9800)
            / 256,
        max_track_speed: max_speed,
        air_drag: u32::from(drag).wrapping_add(3_u32.wrapping_mul(u32::from(drag)) / 20),
        last_speed: input.speed,
        trip_occupancy: if input.capacity == 0 { 100 } else { 0 },
        user_def_data: spec.user_def_data,
        tilt: input.info.misc_flags & 1 != 0,
        curve_speed_mod: spec.curve_speed_mod,
        max_curve_speed: u16::MAX,
        override_present: false,
        colourmap: 0,
    })
}
