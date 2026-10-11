//! Native vanilla cargo distance and fixed-point payment arithmetic.
use super::{State, WorldTickError, unsupported};
use crate::content::CargoSpec;

pub(super) fn distance(state: &State<'_>, packet: u32, tile: u32) -> Result<u32, WorldTickError> {
    let width = std::num::NonZeroU32::new(state.width)
        .ok_or_else(|| unsupported("cargo_payment", "map width"))?;
    let source = u32::try_from(state.number(b"CAPA", packet, "source_xy")?)
        .map_err(|_| unsupported("cargo_payment", "source tile"))?;
    if source == u32::MAX {
        return Err(unsupported("cargo_payment", "unloaded source coordinate"));
    }
    let mut travelled = 0_u32;
    for (name, coordinate) in [("travelled.x", tile % width), ("travelled.y", tile / width)] {
        let saved = i16::try_from(state.number(b"CAPA", packet, name)?)
            .map_err(|_| unsupported("cargo_payment", "travelled vector"))?;
        let coordinate = i16::try_from(coordinate)
            .map_err(|_| unsupported("cargo_payment", "tile coordinate"))?;
        travelled =
            travelled.wrapping_add(u32::from(saved.wrapping_sub(coordinate).unsigned_abs()));
    }
    Ok(travelled.min(
        (source % width)
            .abs_diff(tile % width)
            .wrapping_add((source / width).abs_diff(tile / width)),
    ))
}

pub(super) fn calculate(
    spec: &CargoSpec,
    count: u32,
    distance: u32,
    age: u16,
) -> Result<i64, WorldTickError> {
    if spec.bitnum == 255 {
        return Ok(0);
    }
    if spec.callback_mask & 1 != 0 {
        return Err(unsupported("cargo_payment", "cargo profit callback"));
    }
    let first = i32::from(spec.transit_periods[0]);
    let second = i32::from(spec.transit_periods[1]);
    let elapsed = i32::from(age).wrapping_sub(first);
    let over1 = elapsed.max(0);
    let over2 = over1.wrapping_sub(second).max(0);
    let overmax = (-224_i32).wrapping_add(if second > 224 {
        elapsed
    } else {
        elapsed.wrapping_mul(2).wrapping_sub(second)
    });
    let (factor, shift) = if overmax > 0 {
        let divisor = std::num::NonZeroI32::new(overmax.wrapping_add(32))
            .ok_or_else(|| unsupported("cargo_payment", "transit divisor"))?;
        (
            15872_i32
                .checked_div(divisor.get())
                .ok_or_else(|| unsupported("cargo_payment", "time factor division"))?
                .max(1),
            25,
        )
    } else {
        (255_i32.wrapping_sub(over1).wrapping_sub(over2).max(31), 21)
    };
    let factor = u32::try_from(factor).map_err(|_| unsupported("cargo_payment", "time factor"))?;
    // BigMulS takes signed int32 arguments after native unsigned multiplication.
    let a = i32::from_ne_bytes(
        distance
            .wrapping_mul(factor)
            .wrapping_mul(count)
            .to_ne_bytes(),
    );
    let b = low_i32(spec.current_payment);
    Ok(i64::from(low_i32(
        i64::from(a).wrapping_mul(i64::from(b)) >> shift,
    )))
}
const fn low_i32(value: i64) -> i32 {
    let bytes = value.to_le_bytes();
    i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(test)]
#[path = "income_tests.rs"]
mod tests;
