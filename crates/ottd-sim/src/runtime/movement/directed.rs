//! Directed vanilla road controller; unsupported reached choices roll back the horizon.
mod depot;
mod orders;
use super::{
    RoadPhases, SavedVehicleView, State, VehicleId, WorldTickError, number, set, unsupported,
};
use ottd_save::WireValue;

pub(super) fn admit(v: SavedVehicleView<'_>) -> Result<(), WorldTickError> {
    if !matches!(v.engine_id()?, 116 | 123)
        || v.stored_count()? != 0
        || !matches!(v.common_number("vehstatus")?, 8 | 9)
        || v.setting("vehicle.roadveh_acceleration_model")? != 0
        || v.setting("vehicle.road_side")? != 1
        || v.setting("economy.timekeeping_units")? != 0
    {
        return Err(unsupported("road_route", "vanilla directed road profile"));
    }
    for name in [
        "next",
        "next_shared",
        "breakdown_ctr",
        "vehicle_flags",
        "load_unload_ticks",
        "waiting_triggers",
    ] {
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
    if !matches!(v.common_number("current_order.type")?, 0 | 33) {
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
    if number(state, id, true, "state")? == 254 {
        depot::leave(runner, state)?;
    } else {
        depot::advance(runner, state)?;
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
