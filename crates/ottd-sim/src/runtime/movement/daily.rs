use super::{
    ContentCatalog, SavedVehicleView, State, VehicleId, VehicleSpec, WorldTickError, number,
    random, set, unsupported,
};
const BREAKDOWN_CHANCE: [i64; 64] = [
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 13, 13, 13, 13,
    14, 15, 16, 17, 19, 21, 25, 28, 31, 34, 37, 40, 44, 48, 52, 56, 60, 64, 68, 72, 80, 90, 100,
    110, 120, 130, 140, 150, 170, 190, 210, 230, 250, 250, 250,
];
pub(super) fn run(
    state: &mut State<'_>,
    id: VehicleId,
    content: &ContentCatalog,
) -> Result<(), WorldTickError> {
    let age = number(state, id, false, "economy_age")?.saturating_add(1);
    if age > 730 {
        return Err(unsupported("road_day", "profit group age transition"));
    }
    set(state, id, false, "economy_age", age)?;
    let day = number(state, id, false, "day_counter")?.wrapping_add(1) & 255;
    set(state, id, false, "day_counter", day)?;
    if day.trailing_zeros() >= 3 {
        let previous = number(state, id, false, "value")?;
        set(
            state,
            id,
            false,
            "value",
            previous.wrapping_sub(previous >> 8),
        )?;
    }
    let breakdowns = state.number(b"PATS", 0, "difficulty.vehicle_breakdowns")?;
    if state.number(b"PATS", 0, "order.no_servicing_if_no_breakdowns")? == 0 || breakdowns != 0 {
        let rel = number(state, id, false, "reliability")?
            .saturating_sub(number(state, id, false, "reliability_spd_dec")?)
            .max(0);
        set(state, id, false, "reliability", rel)?;
    }
    if breakdowns >= 1 && number(state, id, false, "cur_speed")? >= 5 {
        let r = random(state)?;
        let chance = number(state, id, false, "breakdown_chance")?
            .saturating_add(1)
            .saturating_add(
                if (((r & 65535).wrapping_mul(25).wrapping_add(12)) >> 16) < 1 {
                    25
                } else {
                    0
                },
            )
            .min(255);
        set(state, id, false, "breakdown_chance", chance)?;
        let rel = number(state, id, false, "reliability")?.saturating_add(if breakdowns == 1 {
            0x6666
        } else {
            0
        });
        let threshold = BREAKDOWN_CHANCE
            .get(
                usize::try_from(rel.min(65535) >> 10)
                    .map_err(|_| unsupported("road_day", "reliability"))?,
            )
            .ok_or_else(|| unsupported("road_day", "reliability index"))?;
        if *threshold <= chance {
            return Err(unsupported("road_day", "reached breakdown lifecycle"));
        }
    }
    if number(state, id, false, "date_of_last_service")?.saturating_add(number(
        state,
        id,
        false,
        "service_interval",
    )?) < state.number(b"DATE", 0, "economy_date")?
    {
        return Err(unsupported("road_day", "reached service interval"));
    }
    running_cost(state, id, content)
}
fn running_cost(
    state: &mut State<'_>,
    id: VehicleId,
    content: &ContentCatalog,
) -> Result<(), WorldTickError> {
    let running = number(state, id, false, "running_ticks")?;
    if running == 0 {
        return Ok(());
    }
    let vehicle = SavedVehicleView::candidate(state.view(), id)?;
    let owner = u32::from(vehicle.owner()?);
    let engine = content
        .engines()
        .get(usize::from(vehicle.engine_id()?))
        .ok_or_else(|| unsupported("road_day", "engine"))?;
    let VehicleSpec::Road(spec) = engine.vehicle else {
        return Err(unsupported("road_day", "engine type"));
    };
    let annual = match spec.running_cost_class {
        Some(price) => content
            .prices()
            .get(price)
            .checked_mul(i64::from(spec.running_cost))
            .ok_or_else(|| unsupported("road_day", "running cost overflow"))?,
        None => 0,
    };
    let cost = annual
        .checked_mul(running)
        .ok_or_else(|| unsupported("road_day", "running ticks overflow"))?
        / (365 * 74);
    set(
        state,
        id,
        false,
        "profit_this_year",
        number(state, id, false, "profit_this_year")?
            .checked_sub(cost)
            .ok_or_else(|| unsupported("road_day", "profit overflow"))?,
    )?;
    set(state, id, false, "running_ticks", 0)?;
    let previous = state.number(b"PLYR", owner, "money_fraction")?;
    let fraction = previous.wrapping_sub(cost & 255) & 255;
    state.set_number(b"PLYR", owner, "money_fraction", fraction)?;
    state.debit(
        owner,
        3,
        (cost >> 8).wrapping_add(i64::from(fraction > previous)),
    )
}
