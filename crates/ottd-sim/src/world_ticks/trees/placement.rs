// SPDX-License-Identifier: GPL-2.0-only
//! Native MakeTree/MakeClear raw fields and candidate-visible neighbor placement.
use super::{State, WorldTickError, random};
use crate::{Tile, unsupported_tick as unsupported};
use ottd_save::world::WorldEdit;

pub(super) fn neighbor(
    state: &mut State<'_>,
    species: u8,
    index: u32,
) -> Result<(), WorldTickError> {
    let (dx, dy): (i64, i64) = match random(state)? & 7 {
        0 => (-1, -1),
        1 => (-1, 0),
        2 => (-1, 1),
        3 => (0, 1),
        4 => (1, 1),
        5 => (1, 0),
        6 => (1, -1),
        _ => (0, -1),
    };
    let next = i64::from(index)
        .wrapping_add(dy.wrapping_mul(i64::from(state.width)))
        .wrapping_add(dx);
    let next = u32::try_from(next)
        .map_err(|_| unsupported("trees", "native neighbor tile assertion boundary"))?;
    if next >= state.width.wrapping_mul(state.height) {
        return Err(unsupported(
            "trees",
            "native neighbor tile assertion boundary",
        ));
    }
    plant(state, next, species, true)
}

pub(super) fn plant(
    state: &mut State<'_>,
    index: u32,
    species: u8,
    neighbor: bool,
) -> Result<(), WorldTickError> {
    let mut tile = super::super::tiles::tile(&state.view().tile(index)?);
    let ground = (tile.m5 >> 2) & 7;
    if tile.tile_type >> 4 != 0 || tile.tile_type & 0x0c != 0 || ground > 1 {
        return Ok(());
    }
    let density = if ground == 1 { 3 } else { tile.m5 & 3 };
    if neighbor && ground == 0 && density != 3 {
        return Ok(());
    }
    tile.tile_type = (tile.tile_type & 15) | 0x40;
    tile.m1 = (tile.m1 & 0x80) | 0x70;
    tile.m2 = u16::from(ground << 6 | density << 4);
    tile.m3 = species;
    tile.m4 = 0;
    tile.m5 = 0;
    tile.m6 &= 3;
    tile.m7 = 0;
    tile.m8 = 0;
    state.apply(WorldEdit::Tile {
        index,
        value: super::super::tiles::saved(&tile),
    })
}

pub(super) const fn clear(tile: &mut Tile) {
    let ground = (tile.m2 >> 6) & 7;
    let density = if ground == 1 { 3 } else { (tile.m2 >> 4) & 3 };
    tile.tile_type &= 15;
    tile.m1 = 16;
    tile.m2 = 0;
    tile.m3 = 0;
    tile.m4 = 0;
    tile.m5 = (ground << 2 | density).to_le_bytes()[0];
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
}
