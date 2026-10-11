//! The source-defined no-service branch with no pending company replacements.
use super::{State, VehicleId, WorldTickError, number, unsupported};
use crate::runtime::transport::cargo::read::Read;

pub(super) fn check(state: &State<'_>, id: VehicleId) -> Result<(), WorldTickError> {
    if number(state, id, false, "date_of_last_service")?.saturating_add(number(
        state,
        id,
        false,
        "service_interval",
    )?) >= state.number(b"DATE", 0, "economy_date")?
    {
        return Ok(());
    }
    if state.number(b"PATS", 0, "order.no_servicing_if_no_breakdowns")? != 0
        && state.number(b"PATS", 0, "difficulty.vehicle_breakdowns")? == 0
    {
        let owner = u32::try_from(number(state, id, false, "owner")?)
            .map_err(|_| unsupported("road_day", "service owner"))?;
        let settings = Read::Candidate(state.view())
            .row(*b"PLYR", owner)?
            .child("settings")?;
        if settings.number("engine_renew_list")? == 0 {
            return Ok(());
        }
    }
    Err(unsupported(
        "road_day",
        "reached service or replacement lifecycle",
    ))
}
