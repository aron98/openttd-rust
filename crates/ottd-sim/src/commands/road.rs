#[cfg(test)]
mod slope;

use super::{Command, CommandCost, CommandError, Plan, landscape};
use crate::world_access::unsigned;
use ottd_save::{
    TileRawParts,
    world::{World, WorldEdit},
};

pub(super) fn build(world: &World, company: u8, command: &Command) -> Result<Plan, CommandError> {
    let Command::BuildRoad {
        tile,
        pieces,
        road_type,
        toggle_disallowed,
        town_id,
    } = *command
    else {
        return Err(CommandError::Unsupported("road command arguments"));
    };
    if town_id != u16::MAX || pieces == 0 || pieces > 15 || toggle_disallowed > 3 {
        return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
    }
    if road_type != 0 {
        return Err(CommandError::Unsupported("non-vanilla road type"));
    }
    let source = landscape::tile_at(world, tile)?;
    let width = world.map().width();
    if [
        tile.checked_add(1),
        tile.checked_add(width),
        tile.checked_add(width).and_then(|n| n.checked_add(1)),
    ]
    .into_iter()
    .any(|i| {
        i.and_then(|i| landscape::tile_at(world, i).ok())
            .is_none_or(|t| t.height() != source.height())
    }) {
        return Err(CommandError::Unsupported("road slopes"));
    }
    let mut existing = 0;
    let mut edits = Vec::new();
    let mut cost = 0;
    let mut parts = TileRawParts::from(source);
    match source.tile_type() >> 4 {
        0 => {
            let clear = landscape::clear(world, company, tile, true)?;
            if !clear.cost.success {
                return Ok(clear);
            }
            cost = clear.cost.cost;
            edits.extend(
                clear
                    .edits
                    .into_iter()
                    .filter(|e| !matches!(e,WorldEdit::Tile {index,..} if *index==tile)),
            );
            parts = TileRawParts {
                tile_type: (source.tile_type() & 15) | 32,
                height: source.height(),
                m1: company,
                m2: nearest_town(world, tile)?,
                m3: company << 4,
                m4: 0,
                m5: pieces,
                m6: 0,
                m7: 0,
                m8: 63 << 6,
            };
        }
        2 => {
            if let Some(plan) = normal_road(world, company, (tile, pieces, toggle_disallowed))? {
                return Ok(plan);
            }
            existing = source.m5() & 15;
        }
        _ => return Err(CommandError::Unsupported("road construction tile type")),
    }
    let added = pieces & !existing;
    cost = cost
        .checked_add(
            i64::from(added.count_ones())
                .checked_mul(landscape::price(world, 95)?)
                .ok_or(CommandError::Overflow("road cost"))?,
        )
        .ok_or(CommandError::Overflow("road cost"))?;
    let joined = existing | pieces;
    let direction = if straight(joined) {
        ((parts.m5 >> 4) & 3) ^ toggle_disallowed
    } else {
        0
    };
    parts.m5 = (parts.m5 & 0xC0) | joined | (direction << 4);
    edits.push(WorldEdit::Tile {
        index: tile,
        value: parts.into(),
    });
    Ok(Plan {
        returns: None,
        cost: CommandCost::success(cost, 0),
        edits,
    })
}
fn normal_road(
    world: &World,
    company: u8,
    args: (u32, u8, u8),
) -> Result<Option<Plan>, CommandError> {
    let (tile, pieces, toggle_disallowed) = args;
    let source = landscape::tile_at(world, tile)?;
    let mut parts = TileRawParts::from(source);
    let mut edits = Vec::new();
    if source.m5() >> 6 != 0 || (source.m8() >> 6) & 63 != 63 || source.m4() & 63 != 0 {
        return Err(CommandError::Unsupported("non-normal road or tram"));
    }
    if (source.m6() >> 3) & 7 >= 6 {
        return Ok(Some(Plan::empty(CommandCost::failure(
            "STR_ERROR_ROAD_WORKS_IN_PROGRESS",
        ))));
    }
    let existing = source.m5() & 15;
    let joined = existing | pieces;
    if ((source.m5() >> 4) & 3 != 0 || toggle_disallowed != 0) && !straight(joined) {
        return Ok(Some(Plan::empty(CommandCost::failure(
            "STR_ERROR_ONEWAY_ROADS_CAN_T_HAVE_JUNCTION",
        ))));
    }
    if existing & pieces == pieces {
        if toggle_disallowed == 0 {
            return Ok(Some(Plan::empty(CommandCost::failure(
                "STR_ERROR_ALREADY_BUILT",
            ))));
        }
        let owner = source.m1() & 31;
        if owner != 16 && owner != company {
            let mut error = CommandCost::failure("STR_ERROR_OWNED_BY");
            error.error_params = match owner {
                0..=14 => vec![0x881D, i64::from(owner)],
                15 => vec![0x8827, i64::from(source.m2())],
                _ => {
                    return Err(CommandError::Unsupported(
                        "non-company road ownership error",
                    ));
                }
            };
            return Ok(Some(Plan::empty(error)));
        }
        let old = (source.m5() >> 4) & 3;
        if old.count_ones() <= (old ^ toggle_disallowed).count_ones() {
            if let Some(error) = super::occupancy::occupied(world, tile)? {
                return Ok(Some(Plan::empty(error)));
            }
        }
        if straight(existing) {
            parts.m5 ^= toggle_disallowed << 4;
            edits.push(WorldEdit::Tile {
                index: tile,
                value: parts.into(),
            });
        }
        return Ok(Some(Plan {
            returns: None,
            cost: CommandCost::success(0, 255),
            edits,
        }));
    }
    if let Some(error) = super::occupancy::occupied(world, tile)? {
        return Ok(Some(Plan::empty(error)));
    }
    Ok(None)
}
const fn straight(pieces: u8) -> bool {
    pieces == 5 || pieces == 10
}
fn nearest_town(world: &World, tile: u32) -> Result<u16, CommandError> {
    let mut nearest = (u64::MAX, u16::MAX);
    let width = std::num::NonZeroU64::new(u64::from(world.map().width()))
        .ok_or(CommandError::Unsupported("zero map width"))?;
    let (x, y) = (u64::from(tile) % width, u64::from(tile) / width);
    if let Some(table) = world.tables().get(b"CITY") {
        for id in table.records().keys() {
            let pos = unsigned(world, b"CITY", *id, "xy")?;
            let distance = x
                .abs_diff(pos % width)
                .checked_add(y.abs_diff(pos / width))
                .ok_or(CommandError::Overflow("town distance"))?;
            let id = u16::try_from(*id).map_err(|_| CommandError::Overflow("town id"))?;
            nearest = nearest.min((distance, id));
        }
    }
    Ok(nearest.1)
}
