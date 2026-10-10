use super::{occupancy, planning, receipt};
use crate::{
    commands::{
        CommandCost, CommandError, CommandReceipt, landscape, pipeline, terrain_read::TerrainRead,
    },
    content::Price,
    runtime::DepotContext,
    world_access::{field_edit, signed, unsigned},
};
use ottd_save::{WireValue, world::World};

pub(in crate::commands) fn run(
    world: &mut World,
    company: u8,
    tile: u32,
    estimate: bool,
    context: DepotContext<'_>,
) -> Result<CommandReceipt, CommandError> {
    let source = landscape::tile_at(world, tile)?;
    if source.tile_type() >> 4 != 2 || source.m5() >> 6 != 2 {
        return Err(CommandError::Unsupported(
            "runtime clearing non-depot terrain",
        ));
    }
    let limit = unsigned(world, b"PLYR", u32::from(company), "clear_limit")?;
    let mut test = if limit >> 16 == 0 {
        CommandCost::failure("STR_ERROR_CLEARING_LIMIT_REACHED")
    } else if source.m1() & 31 != company {
        planning::ownership_error(world, tile, source.m1() & 31)?
    } else if let Some(error) = occupancy::occupied(world, tile)? {
        error
    } else {
        CommandCost::success(context.content.price(Price::ClearDepotRoad), 0)
    };
    // LandscapeClear adds the tile callback cost to its construction accumulator.
    if limit >> 16 != 0 {
        test.expenses = 0;
    }
    if !test.success || estimate {
        return Ok(receipt(test.clone(), test, false));
    }
    if test.cost > 0
        && unsigned(world, b"PATS", 0, "difficulty.infinite_money")? == 0
        && test.cost > signed(world, b"PLYR", u32::from(company), "money")?
    {
        let mut result = test.clone();
        result.success = false;
        result.error = Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY".into());
        result.error_params = vec![result.cost];
        return Ok(receipt(test, result, false));
    }
    let mut edits = landscape::clear_square(TerrainRead::Committed(world), tile)?;
    edits.push(field_edit(
        *b"PLYR",
        u32::from(company),
        "clear_limit",
        WireValue::Unsigned(
            limit
                .checked_sub(65_536)
                .ok_or(CommandError::Overflow("depot clear limit"))?,
        ),
    ));
    edits.extend(pipeline::completion_edits(
        TerrainRead::Committed(world),
        u32::from(company),
        tile,
        &test,
    )?);
    context.publish_removal(world, tile, edits)?;
    Ok(receipt(test.clone(), test, true))
}
