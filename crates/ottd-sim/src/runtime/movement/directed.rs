//! Directed vanilla road controller; unsupported reached choices roll back the horizon.
mod depot;
mod frames;
mod motion;
mod orders;
mod routing;
mod terminal;
mod terminal_far;
mod terminal_near;
use super::{
    RoadPhases, SavedVehicleView, State, VehicleId, WorldTickError, number, set, unsupported,
};
use ottd_save::WireValue;

pub(super) fn admit(v: SavedVehicleView<'_>) -> Result<(), WorldTickError> {
    if !matches!(v.engine_id()?, 116 | 119 | 123)
        || v.stored_count()? != 0
        || !matches!(v.common_number("vehstatus")?, 8 | 9)
        || v.setting("vehicle.roadveh_acceleration_model")? != 0
        || v.setting("vehicle.road_side")? != 1
        || v.setting("economy.timekeeping_units")? != 0
    {
        return Err(unsupported("road_route", "vanilla directed road profile"));
    }
    for name in ["next", "next_shared", "breakdown_ctr", "waiting_triggers"] {
        if v.common_number(name)? != 0 {
            return Err(unsupported("road_route", name));
        }
    }
    for name in [
        "blocked_ctr",
        "overtaking",
        "overtaking_ctr",
        "reverse_ctr",
        "crashed_ctr",
    ] {
        if !matches!(v.road_field(name)?, WireValue::Unsigned(0)) {
            return Err(unsupported("road_route", name));
        }
    }
    if !matches!(v.common_number("current_order.type")?, 0 | 33 | 195 | 4)
        || v.common_number("vehicle_flags")? & !0x0109 != 0
        || !matches!(v.common_number("load_unload_ticks")?, 0 | 1 | 20)
    {
        return Err(unsupported("road_route", "loading/order lifecycle"));
    }
    Ok(())
}

pub(super) fn tick(
    runner: &mut RoadPhases<'_>,
    state: &mut State<'_>,
) -> Result<(), WorldTickError> {
    let id = runner.id;
    for name in ["tick_counter", "running_ticks"] {
        set(
            state,
            id,
            false,
            name,
            number(state, id, false, name)?.wrapping_add(1) & 255,
        )?;
    }
    set(
        state,
        id,
        false,
        "current_order_time",
        number(state, id, false, "current_order_time")?.wrapping_add(1) & 0xffff_ffff,
    )?;
    orders::process(state, id, runner.content)?;
    super::loading::handle(state, runner)?;
    if number(state, id, false, "current_order.type")? & 15 == 3 {
        // The controller returns while loading; the outer vehicle tick still ages cargo.
    } else if number(state, id, true, "state")? == 254 {
        depot::leave(runner, state)?;
    } else {
        motion::advance(runner, state)?;
    }
    super::stations::age_cargo(state, id, &runner.cache)?;
    set(
        state,
        id,
        false,
        "motion_counter",
        number(state, id, false, "motion_counter")?.wrapping_add(number(
            state,
            id,
            false,
            "cur_speed",
        )?) & 0xffff_ffff,
    )?;
    Ok(())
}
