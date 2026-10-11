// SPDX-License-Identifier: GPL-2.0-only
//! Temperate tree tile callbacks and post-vehicle placement from OpenTTD 15.3.
#![expect(
    clippy::verbose_bit_mask,
    reason = "masks preserve the native saved bitfield and RNG branch definitions"
)]
mod placement;
use super::{State, WorldTickError};
use crate::Tile;

fn random(state: &mut State<'_>) -> Result<u32, WorldTickError> {
    let mut rng = ottd_core::Randomizer::from_state(state.random_state()?);
    let draw = rng.next_u32();
    for (name, word) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(rng.state())
    {
        state.set_number(b"DATE", 0, name, i64::from(word))?;
    }
    Ok(draw)
}

pub(super) fn visit(
    state: &mut State<'_>,
    tile: &mut Tile,
    index: u32,
    tick: u64,
) -> Result<(), WorldTickError> {
    let x = index & state.width.wrapping_sub(1);
    let y = index >> state.width.trailing_zeros();
    let cycle =
        u64::from(x.wrapping_mul(11).wrapping_add(y.wrapping_mul(9))).wrapping_add(tick >> 8);
    if cycle & 7 == 7 && (tile.m2 >> 6) & 7 == 0 {
        let density = (tile.m2 >> 4) & 3;
        if density < 3 {
            tile.m2 = (tile.m2 & !0x30) | (density.wrapping_add(1) << 4);
            tile.m1 |= 0x60;
        }
    }
    let setting = state.number(b"PATS", 0, "construction.extra_tree_placement")?;
    if setting == 3 || cycle & 15 != 15 {
        return Ok(());
    }
    match tile.m5 & 7 {
        3 => match random(state)? & 7 {
            0 => tile.m5 = tile.m5.wrapping_add(1),
            1 if setting == 2 && tile.m5 >> 6 < 3 => {
                tile.m5 = tile.m5.wrapping_add(64) & !7;
            }
            1 | 2 if setting == 2 => placement::neighbor(state, tile.m3, index)?,
            _ => {}
        },
        6 if setting != 2 => tile.m5 &= !7,
        6 if tile.m5 >> 6 > 0 => tile.m5 = (tile.m5.wrapping_sub(64) & !7) | 3,
        6 => placement::clear(tile),
        _ => tile.m5 = tile.m5.wrapping_add(1),
    }
    Ok(())
}

const fn scale(width: u32, height: u32, amount: u32) -> u32 {
    amount
        .wrapping_mul(width.wrapping_mul(height) >> 12)
        .div_ceil(16)
}

const fn decrement(old: u8, amount: u32) -> (u8, bool) {
    // Native uint8 subtraction intentionally truncates 256 to zero on a 4096² map.
    let next = old.wrapping_sub(amount.to_le_bytes()[0]);
    (next, old <= next)
}

pub(super) fn tick(state: &mut State<'_>, tick: u64) -> Result<(), WorldTickError> {
    let setting = state.number(b"PATS", 0, "construction.extra_tree_placement")?;
    if setting == 0 || setting == 3 {
        return Ok(());
    }
    let skip = scale(state.width, state.height, 16);
    if skip < 16 && tick & u64::from(16_u32.wrapping_div(skip).wrapping_sub(1)) != 0 {
        return Ok(());
    }
    let old = u8::try_from(state.number(b"DATE", 0, "trees_tick_counter")?)
        .map_err(|_| super::unsupported("trees", "counter outside native byte range"))?;
    let (next, underflow) = decrement(old, scale(state.width, state.height, 1));
    state.set_number(b"DATE", 0, "trees_tick_counter", i64::from(next))?;
    if !underflow || setting == 1 {
        return Ok(());
    }
    let draw = random(state)?;
    let index = draw & state.width.wrapping_mul(state.height).wrapping_sub(1);
    let species = u8::try_from((draw >> 24).wrapping_mul(12) / 256)
        .map_err(|_| super::unsupported("trees", "temperate species"))?;
    placement::plant(state, index, species, false)
}

#[cfg(test)]
mod counter_tests;
