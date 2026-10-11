//! Flat vanilla depot release and directed departure frames.
use super::{
    RoadPhases, SavedVehicleView, State, VehicleId, WorldTickError, number, set, unsupported,
};
fn track(direction: u8) -> Result<i64, WorldTickError> {
    match direction {
        0 => Ok(0),
        1 => Ok(1),
        2 => Ok(8),
        3 => Ok(9),
        _ => Err(unsupported("road_route", "depot direction")),
    }
}
fn position(track: i64, frame: i64) -> Result<(i64, i64), WorldTickError> {
    if !(0..16).contains(&frame) {
        return Err(unsupported("road_route", "depot departure frame"));
    }
    match track {
        0 => Ok((15_i64.wrapping_sub(frame), 9)),
        1 => Ok((9, frame)),
        8 => Ok((frame, 5)),
        9 => Ok((5, 15_i64.wrapping_sub(frame))),
        _ => Err(unsupported("road_route", "straight departure track")),
    }
}
fn update_position(
    runner: &mut RoadPhases<'_>,
    state: &mut State<'_>,
) -> Result<(), WorldTickError> {
    let id = runner.id;
    let (x, y) = location(state, id)?;
    set(state, id, false, "x_pos", x)?;
    set(state, id, false, "y_pos", y)?;
    runner
        .spatial
        .update(SavedVehicleView::candidate(state.view(), id)?, state.width)
}
fn location(state: &State<'_>, id: VehicleId) -> Result<(i64, i64), WorldTickError> {
    let tile = number(state, id, false, "tile")?;
    let (x, y) = position(
        number(state, id, true, "state")?,
        number(state, id, true, "frame")?,
    )?;
    let width = i64::from(state.width);
    Ok((
        (tile & width.wrapping_sub(1))
            .wrapping_mul(16)
            .wrapping_add(x),
        (tile >> width.trailing_zeros())
            .wrapping_mul(16)
            .wrapping_add(y),
    ))
}
fn depot(state: &State<'_>, id: VehicleId) -> Result<u8, WorldTickError> {
    let vehicle = SavedVehicleView::candidate(state.view(), id)?;
    let tile = state.view().tile(vehicle.tile()?)?;
    if tile.tile_type() >> 4 != 2
        || tile.m5() >> 6 != 2
        || tile.m4() & 63 != 0
        || tile.m1() & 31 != vehicle.owner()?
    {
        return Err(unsupported("road_route", "owned vanilla depot departure"));
    }
    for offset in [1, state.width, state.width.wrapping_add(1)] {
        if state
            .view()
            .tile(vehicle.tile()?.wrapping_add(offset))?
            .height()
            != tile.height()
        {
            return Err(unsupported("road_route", "flat depot"));
        }
    }
    if number(state, id, false, "z_pos")? != i64::from(tile.height()).wrapping_mul(8) {
        return Err(unsupported("road_route", "depot height"));
    }
    Ok(tile.m5() & 3)
}
pub(super) fn leave(
    runner: &mut RoadPhases<'_>,
    state: &mut State<'_>,
) -> Result<(), WorldTickError> {
    let id = runner.id;
    let direction = depot(state, id)?;
    let engine = u32::from(SavedVehicleView::candidate(state.view(), id)?.engine_id()?);
    for (field, clock) in [
        ("date_of_last_service", "economy_date"),
        ("date_of_last_service_newgrf", "date"),
    ] {
        set(state, id, false, field, state.number(b"DATE", 0, clock)?)?;
    }
    set(state, id, false, "breakdowns_since_last_service", 0)?;
    set(
        state,
        id,
        false,
        "reliability",
        state.number(b"ENGN", engine, "reliability")?,
    )?;
    let chance = if state.number(b"PATS", 0, "difficulty.vehicle_breakdowns")? == 1 {
        0
    } else {
        number(state, id, false, "breakdown_chance")? / 4
    };
    set(state, id, false, "breakdown_chance", chance)?;
    set(
        state,
        id,
        false,
        "vehstatus",
        number(state, id, false, "vehstatus")? & !1,
    )?;
    set(
        state,
        id,
        false,
        "direction",
        i64::from(direction).wrapping_mul(2).wrapping_add(1),
    )?;
    set(state, id, true, "state", track(direction)?)?;
    set(state, id, true, "frame", 6)?;
    set(state, id, false, "cur_speed", 0)?;
    update_position(runner, state)
}
