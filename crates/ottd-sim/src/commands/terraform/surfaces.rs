use super::{
    CommandError, Failure,
    candidate::{Candidate, neighbor},
    landscape, native,
};
use crate::terrain::tile_slope_z;
use ottd_save::{
    TileRawParts,
    world::{World, WorldEdit},
};
use std::collections::BTreeMap;

fn tunnel_in_way(world: &World, tile: u32, z: u8) -> Result<bool, CommandError> {
    let width = std::num::NonZeroU32::new(world.map().width())
        .ok_or(CommandError::Overflow("map width"))?;
    let (x, y) = (tile % width, tile / width);
    let directions = [
        if x > width.get().saturating_sub(1) / 2 {
            0_u8
        } else {
            2
        },
        if y > world.map().height().saturating_sub(1) / 2 {
            3
        } else {
            1
        },
    ];
    for direction in directions {
        let mut cursor = tile;
        while let Some(next) = neighbor(world, cursor, direction ^ 2) {
            cursor = next;
            let source = landscape::tile_at(world, cursor)?;
            if source.tile_type() >> 4 == 7 {
                break;
            }
            let (_, height) = tile_slope_z(world, cursor)
                .map_err(|_| CommandError::Unsupported("invalid tunnel scan geometry"))?;
            if z < height {
                continue;
            }
            if z == height
                && source.tile_type() >> 4 == 9
                && source.m5() & 128 == 0
                && source.m5() & 3 == direction
            {
                return Ok(true);
            }
            break;
        }
    }
    Ok(false)
}
pub(super) fn clear_surfaces(
    state: &Candidate<'_>,
    company: u8,
    up: bool,
    prices: &crate::content::Prices,
) -> Result<(i64, BTreeMap<u32, TileRawParts>), Failure> {
    let world = state.world;
    let width = world.map().width();
    let mut cost = 0_i64;
    let mut tiles = BTreeMap::<u32, TileRawParts>::new();
    for pass in 0..2 {
        for &dirty in &state.dirty {
            let source = landscape::tile_at(world, dirty)?;
            if source.tile_type() >> 4 == 7 {
                continue;
            }
            let corners = [
                state.height(dirty)?,
                state.height(dirty.wrapping_add(1))?,
                state.height(dirty.wrapping_add(width))?,
                state.height(dirty.wrapping_add(width).wrapping_add(1))?,
            ];
            let (_, base) = ottd_core::terrain::slope_from_corners(corners)
                .map_err(|_| CommandError::Unsupported("invalid candidate terrain geometry"))?;
            if pass == 0 {
                if source.tile_type() & 0x0C != 0 {
                    return Err(CommandError::Unsupported("terraform beneath bridge").into());
                }
                if !up && tunnel_in_way(world, dirty, base)? {
                    return Err(native("STR_ERROR_EXCAVATION_WOULD_DAMAGE", dirty));
                }
            }
            let clear = landscape::clear_with_prices(world, company, dirty, true, prices)?;
            if !clear.cost.success {
                return Err(Failure::Native {
                    cost: clear.cost,
                    tile: dirty,
                });
            }
            if pass == 1 {
                cost = cost.saturating_add(clear.cost.cost);
                for edit in clear.edits {
                    match edit {
                        WorldEdit::Tile { index, value } => {
                            tiles.insert(index, TileRawParts::from(&value));
                        }
                        _ => {
                            return Err(CommandError::Unsupported(
                                "unexpected automatic clear object edit",
                            )
                            .into());
                        }
                    }
                }
            }
        }
    }
    Ok((cost, tiles))
}
