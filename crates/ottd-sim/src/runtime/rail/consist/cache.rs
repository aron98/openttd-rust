use super::super::RailVehicleCache;
use super::{
    input::{Part, Settings},
    types::{ConsistError, ConsistRefusal},
};

pub(super) fn part(input: &Part, first_engine: u16, suffix_capacity: u32) -> RailVehicleCache {
    RailVehicleCache {
        id: input.id,
        railtypes: input.spec.railtypes,
        compatible_railtypes: 0,
        first_engine,
        vehicle_length: 8_u8.saturating_sub(input.spec.shorten_factor.min(7)),
        total_length: 0,
        max_speed: 0,
        cargo_age_period: input.info.cargo_age_period,
        visual_effect: if input.spec.railveh_type == 2 {
            64
        } else if input.spec.engclass == 0 {
            20
        } else {
            40
        },
        weight: 0,
        slope_resistance: 0,
        axle_resistance: 0,
        power: 0,
        max_tractive_effort: 0,
        max_track_speed: 0,
        air_drag: 0,
        last_speed: 0,
        trip_occupancy: if suffix_capacity == 0 { 100 } else { 0 },
        user_def_data: input.spec.user_def_data,
        tilt: false,
        curve_speed_mod: 0,
        max_curve_speed: 0,
        override_present: false,
        colourmap: 0,
    }
}

pub(super) fn aggregate(
    chain: &[Part],
    settings: Settings,
    front: &mut RailVehicleCache,
) -> Result<(), ConsistError> {
    let root = chain.first().ok_or(ConsistRefusal::Roster)?;
    let mut weight = 0_u32;
    let mut power = 0_u32;
    let mut max_te = 0_u32;
    let mut max_speed = u16::MAX;
    let mut length = 0_u16;
    let mut curve = i16::MAX;
    let mut tilt = true;
    let mut compatible = 0_u64;
    for part in chain {
        weight = weight.wrapping_add(u32::from(part.spec.weight));
        let part_power = if part.spec.railveh_type == 1 {
            part.spec.power / 2
        } else {
            part.spec.power
        };
        power = power.wrapping_add(u32::from(part_power));
        if part_power > 0 {
            max_te = max_te.wrapping_add(
                u32::from(part.spec.weight).wrapping_mul(u32::from(part.spec.tractive_effort)),
            );
        }
        if part.spec.power > 0 {
            compatible |= 3;
        }
        if (part.spec.railveh_type != 2 || settings.wagon_speed) && part.spec.max_speed != 0 {
            max_speed = max_speed.min(part.spec.max_speed);
        }
        length = length.wrapping_add(u16::from(
            8_u8.saturating_sub(part.spec.shorten_factor.min(7)),
        ));
        curve = curve.min(part.spec.curve_speed_mod);
        tilt &= part.info.misc_flags & 1 != 0;
    }
    let drag = match root.spec.air_drag {
        0 if max_speed <= 10 => 192,
        0 => 2048_u16
            .checked_div(max_speed)
            .ok_or(ConsistRefusal::Limits)?
            .max(1),
        1 => 0,
        value => u16::from(value),
    };
    let parts = u32::try_from(chain.len()).map_err(|_| ConsistRefusal::Limits)?;
    front.weight = weight.max(1);
    let [low, high, _, _] = weight.wrapping_mul(10).to_le_bytes();
    front.axle_resistance = u16::from_le_bytes([low, high]);
    front.power = power;
    front.max_tractive_effort = max_te.wrapping_mul(9800) / 256;
    front.air_drag =
        u32::from(drag).wrapping_add(3_u32.wrapping_mul(u32::from(drag)).wrapping_mul(parts) / 20);
    front.max_speed = max_speed;
    front.max_track_speed = max_speed;
    front.total_length = length;
    front.curve_speed_mod = curve;
    front.max_curve_speed = u16::MAX;
    front.tilt = tilt;
    front.compatible_railtypes = compatible;
    front.last_speed = root.speed;
    Ok(())
}

pub(super) fn stopped_acceleration(front: &RailVehicleCache) -> Result<i32, ConsistError> {
    // GetAcceleration at zero speed with AS_BRAKE and proven clear saved slope bits.
    let mass = i64::from(front.weight);
    let resistance = i64::from(front.axle_resistance).wrapping_add(mass.wrapping_mul(15));
    let force = i64::from(front.power)
        .wrapping_mul(746)
        .max(mass.wrapping_mul(8).wrapping_add(resistance));
    let braking = force
        .wrapping_neg()
        .wrapping_sub(resistance)
        .min(-10000)
        .checked_div(mass)
        .ok_or(ConsistRefusal::Limits)?;
    i32::try_from(braking.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
        .map_err(|_| ConsistRefusal::Limits.into())
}
