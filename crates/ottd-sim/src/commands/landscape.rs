use super::{CommandCost, CommandError, Plan, terrain_read::TerrainRead};
use crate::content::{ContentCatalog, Price, Prices};
use crate::world_access::{field_edit, unsigned};
use ottd_save::{
    TileRawParts, TileState, WireValue,
    world::{World, WorldEdit},
};

pub(super) fn price(world: &World, base: i64) -> Result<i64, CommandError> {
    if world
        .tables()
        .get(b"NGRF")
        .is_some_and(|t| !t.records().is_empty())
    {
        return Err(CommandError::Unsupported("NewGRF construction prices"));
    }
    let factor = match unsigned(world, b"PATS", 0, "difficulty.construction_cost")? {
        0 => 6,
        1 => 8,
        2 => 9,
        _ => return Err(CommandError::Unsupported("construction difficulty")),
    };
    let inflation = i64::try_from(unsigned(world, b"ECMY", 0, "inflation_prices")?)
        .map_err(|_| CommandError::Overflow("price inflation"))?;
    let value = base
        .checked_mul(factor)
        .and_then(|n| n.checked_mul(inflation))
        .ok_or(CommandError::Overflow("construction price"))?
        >> 19;
    Ok(if value == 0 { base.signum() } else { value })
}
pub(super) fn clear(
    world: &World,
    company: u8,
    tile: u32,
    automatic: bool,
) -> Result<Plan, CommandError> {
    let catalog = ContentCatalog::from_world(world)?;
    clear_with_prices(
        TerrainRead::Committed(world),
        company,
        tile,
        automatic,
        catalog.prices(),
    )
}
pub(super) fn clear_with_prices(
    world: TerrainRead<'_>,
    company: u8,
    tile: u32,
    automatic: bool,
    prices: &Prices,
) -> Result<Plan, CommandError> {
    let source = world.tile(tile)?;
    if source.tile_type() >> 4 != 0 {
        return Err(CommandError::Unsupported("clearing non-clear terrain"));
    }
    let company = u32::from(company);
    let limit = world.unsigned(*b"PLYR", company, "clear_limit")?;
    if !automatic && limit >> 16 == 0 {
        return Ok(Plan::empty(CommandCost::failure(
            "STR_ERROR_CLEARING_LIMIT_REACHED",
        )));
    }
    let ground = (source.m5() >> 2) & 7;
    let base = match ground {
        0 => Price::ClearGrass,
        1 | 4 | 5 => Price::ClearRough,
        2 => Price::ClearRocks,
        3 => Price::ClearFields,
        _ => return Err(CommandError::Unsupported("invalid clear ground")),
    };
    let snow = source.m3() & 16 != 0;
    let mut cost = if snow || ground != 0 || source.m5() & 3 != 0 {
        prices.get(base)
    } else {
        0
    };
    if snow {
        cost = cost
            .checked_add(
                prices
                    .get(Price::ClearRough)
                    .checked_sub(prices.get(Price::ClearGrass))
                    .and_then(i64::checked_abs)
                    .ok_or(CommandError::Overflow("snow price"))?,
            )
            .ok_or(CommandError::Overflow("snow clearing"))?;
    }
    let mut edits = clear_square(world, tile)?;
    if !automatic {
        edits.push(field_edit(
            *b"PLYR",
            company,
            "clear_limit",
            WireValue::Unsigned(
                limit
                    .checked_sub(65_536)
                    .ok_or(CommandError::Overflow("clear limit"))?,
            ),
        ));
    }
    Ok(Plan {
        returns: None,
        cost: CommandCost::success(cost, 0),
        edits,
    })
}
pub(super) fn clear_square(
    world: TerrainRead<'_>,
    tile: u32,
) -> Result<Vec<WorldEdit>, CommandError> {
    let source = world.tile(tile)?;
    let clear = TileRawParts {
        tile_type: source.tile_type() & 15,
        height: source.height(),
        m1: 16,
        m2: 0,
        m3: 0,
        m4: 0,
        m5: 0,
        m6: 0,
        m7: 0,
        m8: 0,
    };
    let mut edits = vec![WorldEdit::Tile {
        index: tile,
        value: clear.into(),
    }];
    edits.extend(clear_neighbor_water(world, tile)?);
    Ok(edits)
}

fn clear_neighbor_water(world: TerrainRead<'_>, tile: u32) -> Result<Vec<WorldEdit>, CommandError> {
    let mut edits = Vec::new();
    let width = i64::from(world.size().width());
    let previous = width
        .checked_sub(1)
        .ok_or(CommandError::Overflow("map width"))?;
    let next = width
        .checked_add(1)
        .ok_or(CommandError::Overflow("map width"))?;
    for offset in [
        next.saturating_neg(),
        width.saturating_neg(),
        previous.saturating_neg(),
        -1,
        1,
        previous,
        width,
        next,
    ] {
        let Some(index) = i64::from(tile)
            .checked_add(offset)
            .and_then(|v| u32::try_from(v).ok())
        else {
            continue;
        };
        if index >= world.size().count()? {
            continue;
        }
        let neighbour = world.tile(index)?;
        if neighbour.tile_type() >> 4 == 6 {
            let mut parts = TileRawParts::from(&neighbour);
            parts.m3 &= !1;
            edits.push(WorldEdit::Tile {
                index,
                value: parts.into(),
            });
        }
    }
    Ok(edits)
}
pub(super) fn tile_at(world: &World, tile: u32) -> Result<&TileState, CommandError> {
    usize::try_from(tile)
        .ok()
        .and_then(|index| world.map().tiles().get(index))
        .ok_or(CommandError::Unsupported("tile outside map"))
}
