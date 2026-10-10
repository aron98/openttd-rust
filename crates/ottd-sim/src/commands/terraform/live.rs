use super::{Args, CommandCost, CommandError, Failure, Plan, candidate, native, outcome, surfaces};
use crate::{
    commands::{
        terrain_context::{ClearRequest, TerrainContext, TerrainFlags},
        terrain_state::TerrainState,
        tree_clear,
    },
    world_access::field_edit,
};
use ottd_save::{TileRawParts, WireValue, world::WorldEdit};

pub(in crate::commands) fn body(
    state: &mut TerrainState<'_, '_>,
    context: &mut TerrainContext,
    args: Args,
    flags: TerrainFlags,
) -> Result<Plan, CommandError> {
    match build(state, context, args, flags) {
        Ok(plan) => Ok(plan),
        Err(Failure::Scope(error)) => Err(error),
        Err(Failure::Native { cost, tile }) => Ok(outcome(cost, tile, Vec::new())),
    }
}

fn build(
    state: &mut TerrainState<'_, '_>,
    context: &mut TerrainContext,
    args: Args,
    flags: TerrainFlags,
) -> Result<Plan, Failure> {
    let (geometry, mut cost) = candidate::prepare(state.read(), args, state.prices)?;
    let (heights, dirty_tiles) = (geometry.heights, geometry.dirty);
    let width = state.read().size().width();
    for pass in 0..2 {
        for &tile in &dirty_tiles {
            let world = state.read();
            let source = world.tile(tile)?;
            if source.tile_type() >> 4 == 7 {
                continue;
            }
            let height = |tile| {
                heights
                    .get(&tile)
                    .copied()
                    .map_or_else(|| world.tile(tile).map(|tile| tile.height()), Ok)
            };
            let corners = [
                height(tile)?,
                height(tile.wrapping_add(1))?,
                height(tile.wrapping_add(width))?,
                height(tile.wrapping_add(width).wrapping_add(1))?,
            ];
            let (_, base) = ottd_core::terrain::slope_from_corners(corners)
                .map_err(|_| CommandError::Unsupported("invalid candidate terrain geometry"))?;
            if pass == 0 {
                if source.tile_type() & 0x0C != 0 {
                    return Err(CommandError::Unsupported("terraform beneath bridge").into());
                }
                if !args.up && surfaces::tunnel_in_way(world, tile, base)? {
                    return Err(native("STR_ERROR_EXCAVATION_WOULD_DAMAGE", tile));
                }
            }
            let surface_flags = TerrainFlags(if pass == 0 {
                (flags.0 | 2 | 1024 | 512) & !1
            } else {
                flags.0 | 2 | 1024
            });
            #[cfg(test)]
            context.record(crate::commands::terrain_context::trace::Event::Surface {
                tile,
                flags: surface_flags.0,
                company: context.company,
                pass,
            });
            let request = ClearRequest {
                tile,
                flags: surface_flags,
            };
            let clear = if surface_flags.executing() {
                tree_clear::body(state, context, request)?
            } else {
                context.test(|context| tree_clear::body(state, context, request))?
            };
            if !clear.success {
                return Err(Failure::Native { cost: clear, tile });
            }
            if pass == 1 {
                cost = cost.saturating_add(clear.cost);
            }
        }
    }
    let limit = u32::try_from(state.read().unsigned(
        *b"PLYR",
        u32::from(context.company),
        "terraform_limit",
    )?)
    .map_err(|_| CommandError::Overflow("terraform limit"))?;
    let changed =
        u32::try_from(heights.len()).map_err(|_| CommandError::Overflow("changed corners"))?;
    if (limit >> 16) < changed {
        return Err(native("STR_ERROR_TERRAFORM_LIMIT_REACHED", u32::MAX));
    }
    if flags.executing() {
        let mut edits = Vec::with_capacity(heights.len().saturating_add(1));
        for (tile, height) in heights {
            let mut parts = TileRawParts::from(&state.read().tile(tile)?);
            parts.height = height;
            edits.push(WorldEdit::Tile {
                index: tile,
                value: parts.into(),
            });
        }
        edits.push(field_edit(
            *b"PLYR",
            u32::from(context.company),
            "terraform_limit",
            WireValue::Unsigned(u64::from(limit.wrapping_sub(changed << 16))),
        ));
        state.apply(edits)?;
    }
    Ok(outcome(
        CommandCost::success(cost, 0),
        args.tile,
        Vec::new(),
    ))
}
