#[cfg(test)]
use super::terrain_context::trace::Event;
use super::{
    CommandCost, CommandError, landscape,
    terrain_context::{ClearRequest, TerrainContext},
    terrain_state::TerrainState,
    town_rating,
};
use crate::{content::Price, world_access::field_edit};
use ottd_save::WireValue;

pub(super) fn body(
    state: &mut TerrainState<'_, '_>,
    context: &mut TerrainContext,
    request: ClearRequest,
) -> Result<CommandCost, CommandError> {
    let source = state.read().tile(request.tile)?;
    if source.tile_type() >> 4 != 4 {
        let plan = landscape::clear_with_prices(
            state.read(),
            context.company,
            request.tile,
            request.flags.automatic(),
            state.prices,
        )?;
        if request.flags.executing() && plan.cost.success {
            state.apply(plan.edits)?;
        }
        return Ok(plan.cost);
    }
    let limit = u32::try_from(state.read().unsigned(
        *b"PLYR",
        u32::from(context.company),
        "clear_limit",
    )?)
    .map_err(|_| CommandError::Overflow("clear limit"))?;
    if !request.flags.automatic() && limit >> 16 == 0 {
        return Ok(CommandCost::failure("STR_ERROR_CLEARING_LIMIT_REACHED"));
    }
    #[cfg(test)]
    context.record(Event::Tree {
        tile: request.tile,
        flags: request.flags.0,
        company: context.company,
    });
    town_rating::clear_tree(state, context, request)?;
    let count = i64::from((source.m5() >> 6).saturating_add(1));
    let multiplier = if (20..27).contains(&source.m3()) {
        4_i64
    } else {
        1
    };
    let cost = count
        .checked_mul(multiplier)
        .and_then(|n| n.checked_mul(state.prices.get(Price::ClearTrees)))
        .ok_or(CommandError::Overflow("tree clear price"))?;
    if request.flags.executing() {
        let mut edits = landscape::clear_square(state.read(), request.tile)?;
        if !request.flags.automatic() {
            edits.push(field_edit(
                *b"PLYR",
                u32::from(context.company),
                "clear_limit",
                WireValue::Unsigned(u64::from(
                    limit
                        .checked_sub(65536)
                        .ok_or(CommandError::Overflow("clear limit"))?,
                )),
            ));
        }
        state.apply(edits)?;
    }
    Ok(CommandCost::success(cost, 0))
}
