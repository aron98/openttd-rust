mod candidate;
mod surfaces;
use candidate::Candidate;
use surfaces::clear_surfaces;

use super::{CommandCost, CommandError, CommandReturn, Plan, terrain_read::TerrainRead};
use crate::{
    content::{ContentCatalog, Price, Prices},
    world_access::field_edit,
};
use ottd_save::{
    TileRawParts, WireValue,
    world::{World, WorldEdit},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
enum Failure {
    Scope(CommandError),
    Native { cost: CommandCost, tile: u32 },
}
impl From<CommandError> for Failure {
    fn from(error: CommandError) -> Self {
        Self::Scope(error)
    }
}
fn native(symbol: &str, tile: u32) -> Failure {
    Failure::Native {
        cost: CommandCost::failure(symbol),
        tile,
    }
}
const fn outcome(cost: CommandCost, tile: u32, edits: Vec<WorldEdit>) -> Plan {
    Plan {
        cost,
        edits,
        returns: Some(CommandReturn::Landscape {
            additional_money: 0,
            tile,
        }),
    }
}
pub(super) fn plan(
    world: &World,
    company: u8,
    tile: u32,
    mask: u8,
    up: bool,
) -> Result<Plan, CommandError> {
    let catalog = ContentCatalog::from_world(world)?;
    step(
        TerrainRead::Committed(world),
        company,
        tile,
        mask,
        up,
        catalog.prices(),
    )
}
pub(super) fn step(
    world: TerrainRead<'_>,
    company: u8,
    tile: u32,
    mask: u8,
    up: bool,
    prices: &Prices,
) -> Result<Plan, CommandError> {
    match build(world, company, tile, mask, up, prices) {
        Ok(plan) => Ok(plan),
        Err(Failure::Scope(error)) => Err(error),
        Err(Failure::Native { cost, tile }) => Ok(outcome(cost, tile, Vec::new())),
    }
}

fn build(
    world: TerrainRead<'_>,
    company: u8,
    tile: u32,
    mask: u8,
    up: bool,
    prices: &Prices,
) -> Result<Plan, Failure> {
    let width = world.size().width();
    let count = world.size().count()?;
    let mut state = Candidate {
        world,
        heights: BTreeMap::new(),
        dirty: BTreeSet::new(),
        freeform: world.unsigned(*b"PATS", 0, "construction.freeform_edges")? != 0,
        maximum: world.unsigned(*b"PATS", 0, "construction.map_height_limit")?,
        price: prices.get(Price::Terraform),
    };
    let mut cost = 0_i64;
    for (bit, offset) in [(1, 1), (2, width.saturating_add(1)), (4, width), (8, 0)] {
        if mask & bit == 0 {
            continue;
        }
        let Some(corner) = tile.checked_add(offset).filter(|n| *n < count) else {
            continue;
        };
        let height = i16::from(world.tile(corner)?.height());
        cost = cost.saturating_add(state.change(
            corner,
            if up {
                height.saturating_add(1)
            } else {
                height.saturating_sub(1)
            },
        )?);
    }
    let (surface_cost, mut tiles) = clear_surfaces(&state, company, up, prices)?;
    cost = cost.saturating_add(surface_cost);
    let limit = world.unsigned(*b"PLYR", u32::from(company), "terraform_limit")?;
    let changed = u64::try_from(state.heights.len())
        .map_err(|_| CommandError::Overflow("changed corners"))?;
    if ((limit >> 16) & 65535) < changed {
        return Err(native("STR_ERROR_TERRAFORM_LIMIT_REACHED", u32::MAX));
    }
    for (index, height) in state.heights {
        let parts = tiles
            .entry(index)
            .or_insert(TileRawParts::from(&world.tile(index)?));
        parts.height = height;
    }
    let mut edits: Vec<_> = tiles
        .into_iter()
        .map(|(index, parts)| WorldEdit::Tile {
            index,
            value: parts.into(),
        })
        .collect();
    edits.push(field_edit(
        *b"PLYR",
        u32::from(company),
        "terraform_limit",
        WireValue::Unsigned(limit.wrapping_sub(changed << 16)),
    ));
    Ok(outcome(CommandCost::success(cost, 0), tile, edits))
}
