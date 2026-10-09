use super::{RoadVehicleCache, RuntimeError, SavedVehicleView};
use crate::content::{ContentCatalog, VehicleSpec};

pub(super) fn create(
    vehicle: SavedVehicleView<'_>,
    content: &ContentCatalog,
) -> Result<RoadVehicleCache, RuntimeError> {
    let realistic = vehicle.setting("vehicle.roadveh_acceleration_model")? != 0;
    calculate(vehicle, content, realistic, false)
}

pub(super) fn restore(
    vehicle: SavedVehicleView<'_>,
    content: &ContentCatalog,
) -> Result<RoadVehicleCache, RuntimeError> {
    calculate(vehicle, content, true, true)
}
fn calculate(
    vehicle: SavedVehicleView<'_>,
    content: &ContentCatalog,
    acceleration: bool,
    after_load: bool,
) -> Result<RoadVehicleCache, RuntimeError> {
    let engine = content
        .engines()
        .get(usize::from(vehicle.engine_id()?))
        .ok_or(RuntimeError::Invalid("engine_type"))?;
    let spec = match engine.vehicle {
        VehicleSpec::Road(spec) => spec,
        VehicleSpec::Rail(_) | VehicleSpec::Ship(_) | VehicleSpec::Aircraft(_) => {
            return Err(RuntimeError::Invalid("road vehicle engine type"));
        }
    };
    if spec.roadtype != 0 || spec.power == 0 {
        return Err(RuntimeError::Unsupported("custom or powerless road engine"));
    }
    let tile = vehicle.tile_state()?;
    let has_road = match tile.tile_type() >> 4 {
        2 => true,
        5 => matches!((tile.m6() >> 3) & 15, 2 | 3 | 8),
        9 => (tile.m5() >> 2) & 3 == 1,
        _ => false,
    };
    if !has_road || tile.m4() & 63 != 0 {
        return Err(RuntimeError::Unsupported("vehicle tile lacks vanilla road"));
    }
    let cargo = content
        .cargo()
        .get(usize::from(vehicle.cargo_type()?))
        .filter(|cargo| cargo.bitnum != 255)
        .ok_or(RuntimeError::Invalid("vehicle cargo type"))?;
    let stored = vehicle.stored_count()?;
    let cargo_weight = stored.wrapping_mul(u32::from(cargo.weight)) / 16;
    let weight = u16::try_from(cargo_weight & 0xffff)
        .map_err(|_| RuntimeError::Invalid("cargo weight"))?
        .wrapping_add(u16::from(spec.weight) / 4);
    let slope = vehicle.setting("vehicle.roadveh_slope_steepness")?;
    let display_speed = spec.max_speed / 2;
    let drag = match spec.air_drag {
        0 if display_speed <= 10 => 192,
        0 => 2048_u16
            .checked_div(display_speed)
            .ok_or(RuntimeError::Invalid("display speed"))?
            .max(1),
        1 => 0,
        value => u16::from(value),
    };
    let weight = u32::from(weight);
    let length = 8_u8.saturating_sub(spec.shorten_factor.min(7));
    let mut cache = RoadVehicleCache {
        id: vehicle.id(),
        road_type: spec.roadtype,
        compatible_roadtypes: 1,
        first_engine: None,
        vehicle_length: length,
        total_length: u16::from(length),
        max_speed: spec.max_speed,
        cargo_age_period: engine.info.cargo_age_period,
        visual_effect: 64,
        weight: weight.max(1),
        slope_resistance: weight.wrapping_mul(slope).wrapping_mul(100),
        axle_resistance: u16::try_from(weight.wrapping_mul(10) & 0xffff)
            .map_err(|_| RuntimeError::Invalid("axle resistance"))?,
        power: u32::from(spec.power).wrapping_mul(10),
        max_tractive_effort: weight
            .wrapping_mul(u32::from(spec.tractive_effort))
            .wrapping_mul(9800)
            / 256,
        max_track_speed: spec.max_speed,
        air_drag: u32::from(drag).wrapping_add(3_u32.wrapping_mul(u32::from(drag)) / 20),
        last_speed: if after_load {
            vehicle.current_speed()?
        } else {
            0
        },
        trip_occupancy: if after_load {
            occupancy(stored, vehicle.capacity()?)?
        } else {
            0
        },
    };
    if !acceleration {
        cache.weight = 0;
        cache.slope_resistance = 0;
        cache.axle_resistance = 0;
        cache.power = 0;
        cache.max_tractive_effort = 0;
        cache.max_track_speed = 0;
        cache.air_drag = 0;
    }
    Ok(cache)
}

fn occupancy(stored: u32, capacity: u16) -> Result<i8, RuntimeError> {
    if capacity == 0 {
        return Ok(100);
    }
    let stored = i32::try_from(stored).map_err(|_| RuntimeError::Invalid("cargo occupancy"))?;
    let capacity = i32::from(capacity);
    let scaled = stored
        .checked_mul(100)
        .ok_or(RuntimeError::Invalid("cargo occupancy"))?;
    let numerator = if stored
        .checked_mul(2)
        .ok_or(RuntimeError::Invalid("cargo occupancy"))?
        < capacity
    {
        scaled
            .checked_add(capacity.saturating_sub(1))
            .ok_or(RuntimeError::Invalid("cargo occupancy"))?
    } else {
        scaled
    };
    let percentage = u8::try_from(
        numerator
            .checked_div(capacity)
            .ok_or(RuntimeError::Invalid("cargo capacity"))?
            & 0xff,
    )
    .map_err(|_| RuntimeError::Invalid("cargo occupancy"))?;
    Ok(i8::from_ne_bytes([percentage]))
}
