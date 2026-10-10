mod occupancy;
mod planning;
mod record;
pub(super) mod remove;
use super::{CommandCost, CommandError, CommandReceipt, pipeline, terrain_read::TerrainRead};
use crate::{
    runtime::DepotContext,
    world_access::{signed, unsigned},
};
use ottd_save::{
    TileRawParts,
    world::{World, WorldEdit},
};

#[derive(Clone, Copy)]
pub(super) struct Args {
    pub tile: u32,
    pub road_type: u8,
    pub direction: u8,
}

fn receipt(test: CommandCost, result: CommandCost, executed: bool) -> CommandReceipt {
    CommandReceipt {
        posted: result.success,
        gate: None,
        test: Some(test),
        exec: executed.then(|| result.clone()),
        result: Some(result),
        returns: None,
    }
}
pub(super) fn run(
    world: &mut World,
    company: u8,
    args: Args,
    estimate: bool,
    context: DepotContext<'_>,
) -> Result<CommandReceipt, CommandError> {
    let mut plan = planning::validate(world, company, args, &context)?;
    if !plan.cost.success || estimate {
        return Ok(receipt(plan.cost.clone(), plan.cost, false));
    }
    if plan.cost.cost > 0
        && unsigned(world, b"PATS", 0, "difficulty.infinite_money")? == 0
        && plan.cost.cost > signed(world, b"PLYR", u32::from(company), "money")?
    {
        let mut result = plan.cost.clone();
        result.success = false;
        result.error = Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY".into());
        result.error_params = vec![result.cost];
        return Ok(receipt(plan.cost, result, false));
    }
    let (allocation, infrastructure) = if plan.create {
        let mut pool = context.pool.clone();
        let id = pool
            .allocate()
            .map_err(crate::runtime::RuntimeError::from)?;
        let source = plan
            .edits
            .iter()
            .rev()
            .find_map(|edit| match edit {
                WorldEdit::Tile { index, value } if *index == args.tile => Some(value),
                _ => None,
            })
            .ok_or(CommandError::Unsupported("cleared depot tile"))?;
        let mut tile = TileRawParts::from(source);
        tile.tile_type = (tile.tile_type & 15) | 32;
        tile.m1 = (tile.m1 & !31) | company;
        tile.m2 = u16::try_from(id).map_err(|_| CommandError::Overflow("depot ID"))?;
        tile.m3 = company << 4;
        tile.m4 = 63;
        tile.m5 = 128 | args.direction;
        tile.m6 &= 3;
        tile.m7 = company;
        tile.m8 = 63 << 6;
        if args.road_type == 1 {
            tile.m8 = u16::from(args.road_type) << 6;
        } else {
            tile.m4 = args.road_type;
        }
        plan.edits.push(WorldEdit::Tile {
            index: args.tile,
            value: tile.into(),
        });
        plan.edits.push(WorldEdit::InsertRecord {
            chunk: *b"DEPT",
            record: id,
            value: record::build(world, args.tile)?,
        });
        let count = context
            .road
            .get(&company)
            .and_then(|counts| counts.get(usize::from(args.road_type)))
            .ok_or(CommandError::Unsupported("company road infrastructure"))?
            .checked_add(2)
            .ok_or(CommandError::Overflow("depot infrastructure"))?;
        (Some(pool), Some((company, args.road_type, count)))
    } else {
        (None, None)
    };
    plan.edits.extend(pipeline::completion_edits(
        TerrainRead::Committed(world),
        u32::from(company),
        args.tile,
        &plan.cost,
    )?);
    context.publish(world, plan.edits, allocation, infrastructure)?;
    Ok(receipt(plan.cost.clone(), plan.cost, true))
}
