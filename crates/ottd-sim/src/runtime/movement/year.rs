//! Native primary-road annual rotation after company and town yearly callbacks.
use super::{RoadPhases, State, WorldTickError, number, set, unsupported};
use crate::runtime::types::NewsItem;

/// Native `STR_NEWS_VEHICLE_UNPROFITABLE_YEAR` (table/strings.h at the pinned
/// source). The wallclock `..._PERIOD` variant stays refused by the existing
/// movement-domain admission (`economy.timekeeping_units` must be zero).
const STR_NEWS_VEHICLE_UNPROFITABLE_YEAR: u32 = 0x2F9;
/// Native `VEHICLE_PROFIT_MIN_AGE` (`vehicle_func.h`).
const VEHICLE_PROFIT_MIN_AGE: i64 = 730;

/// First playable company, mirroring native single-player load
/// (`SetLocalCompany(GetFirstPlayableCompanyID())`): the lowest human company.
/// The movement domain refuses AI companies at admission, so every saved
/// company is human and the lowest pool id is the local company.
fn local_company(state: &State<'_>) -> Result<u8, WorldTickError> {
    u8::try_from(state.company_ids().into_iter().min().unwrap_or(0))
        .map_err(|_| unsupported("vehicle_year", "local company"))
}

pub(super) fn run(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    let current = number(state, runner.id, false, "profit_this_year")?;
    let display = current >> 8;
    if number(state, runner.id, false, "economy_age")? >= VEHICLE_PROFIT_MIN_AGE && display < 0 {
        let owner = u8::try_from(number(state, runner.id, false, "owner")?)
            .map_err(|_| unsupported("vehicle_year", "vehicle owner"))?;
        if runner.news.vehicle_income_warn() && owner == local_company(state)? {
            runner.news.insert(NewsItem::unprofitable(
                owner,
                runner.id.raw(),
                STR_NEWS_VEHICLE_UNPROFITABLE_YEAR,
                display,
            ))?;
        }
        // Native AI branch (`AI::NewEvent`): human owners drop the event before
        // any storage, so no VM or event queue is involved. AI-company owners
        // remain refused by the movement-domain admission (`company is_ai`).
    }
    set(state, runner.id, false, "profit_last_year", current)?;
    set(state, runner.id, false, "profit_this_year", 0)?;
    runner.group_profits.recount(state.view())?;
    Ok(())
}
