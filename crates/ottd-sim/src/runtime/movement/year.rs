//! Native primary-road annual rotation after company and town yearly callbacks.
use super::{RoadPhases, State, WorldTickError, number, set, unsupported};

pub(super) fn run(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    let current = number(state, runner.id, false, "profit_this_year")?;
    if number(state, runner.id, false, "economy_age")? >= 730 && current >> 8 < 0 {
        return Err(unsupported(
            "vehicle_year",
            "unprofitable warning/news and AI event context",
        ));
    }
    set(state, runner.id, false, "profit_last_year", current)?;
    set(state, runner.id, false, "profit_this_year", 0)?;
    runner.group_profits.recount(state.view())?;
    Ok(())
}
