// SPDX-License-Identifier: GPL-2.0-only
//! Temperate no-spread `TileLoop_Trees`, derived from OpenTTD 15.3 `tree_cmd.cpp`.
#![expect(
    clippy::verbose_bit_mask,
    reason = "masks preserve the native saved bitfield and RNG branch definitions"
)]
use super::{State, WorldTickError};
use crate::Tile;

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
    if state.number(b"PATS", 0, "construction.extra_tree_placement")? == 3 || cycle & 15 != 15 {
        return Ok(());
    }
    match tile.m5 & 7 {
        3 => {
            let mut rng = ottd_core::Randomizer::from_state(state.random_state()?);
            let draw = rng.next_u32();
            for (name, word) in ["random_state[0]", "random_state[1]"]
                .into_iter()
                .zip(rng.state())
            {
                state.set_number(b"DATE", 0, name, i64::from(word))?;
            }
            if draw & 7 == 0 {
                tile.m5 = tile.m5.wrapping_add(1);
            }
        }
        6 => tile.m5 &= !7,
        _ => tile.m5 = tile.m5.wrapping_add(1),
    }
    Ok(())
}
