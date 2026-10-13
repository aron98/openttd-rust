//! `RoadVehController` advance loop and `IndividualRoadVehicleController` road frames.
use super::{
    RoadPhases, SavedVehicleView, State, WorldTickError, frames, number, routing, set, terminal,
    unsupported,
};
fn distance(state: &State<'_>, runner: &RoadPhases<'_>) -> Result<i64, WorldTickError> {
    Ok(if number(state, runner.id, false, "direction")? & 1 != 0 {
        192
    } else {
        256
    })
}
fn turn(
    state: &mut State<'_>,
    runner: &RoadPhases<'_>,
    position: (i64, i64),
) -> Result<bool, WorldTickError> {
    let id = runner.id;
    let old = number(state, id, false, "direction")?;
    let x = position
        .0
        .wrapping_sub(number(state, id, false, "x_pos")?)
        .wrapping_add(1);
    let y = position
        .1
        .wrapping_sub(number(state, id, false, "y_pos")?)
        .wrapping_add(1);
    if !(0..3).contains(&x) || !(0..3).contains(&y) {
        return Ok(false);
    }
    let index = usize::try_from(y.wrapping_mul(4).wrapping_add(x))
        .map_err(|_| unsupported("road_route", "direction delta"))?;
    let target = *[0_i64, 7, 6, 255, 1, 0, 5, 255, 2, 3, 4]
        .get(index)
        .ok_or_else(|| unsupported("road_route", "direction delta"))?;
    if target == old {
        return Ok(false);
    }
    let next = old.wrapping_add(if target.wrapping_sub(old) & 7 > 4 {
        -1
    } else {
        1
    }) & 7;
    set(state, id, false, "direction", next)?;
    let speed = number(state, id, false, "cur_speed")?;
    set(
        state,
        id,
        false,
        "cur_speed",
        speed.wrapping_sub(speed >> 2),
    )?;
    Ok(true)
}
fn step(state: &mut State<'_>, runner: &mut RoadPhases<'_>) -> Result<bool, WorldTickError> {
    let id = runner.id;
    let track = number(state, id, true, "state")?;
    let frame = number(state, id, true, "frame")?;
    let (rx, ry) = frames::at(track, frame.wrapping_add(1))?;
    let old_tile = SavedVehicleView::candidate(state.view(), id)?.tile()?;
    let (tile, next_track, next_frame, local, crossing) = if rx & 0xc0 != 0 {
        let tile = if rx & 0x80 != 0 {
            routing::next(state, runner, rx & 3)?
        } else {
            old_tile
        };
        let next_track = routing::select(state, runner, (tile, rx & 3))?;
        let tile = if matches!(next_track, 6 | 7 | 14 | 15) {
            old_tile
        } else {
            tile
        };
        let next_frame = i64::from(rx & 0x40 != 0);
        (
            tile,
            i64::from(next_track),
            next_frame,
            frames::at(i64::from(next_track), next_frame)?,
            true,
        )
    } else {
        (old_tile, track, frame.wrapping_add(1), (rx, ry), false)
    };
    let width = i64::from(state.width);
    let position = (
        (i64::from(tile) & width.wrapping_sub(1))
            .wrapping_mul(16)
            .wrapping_add(i64::from(local.0)),
        (i64::from(tile) >> width.trailing_zeros())
            .wrapping_mul(16)
            .wrapping_add(i64::from(local.1)),
    );
    if !crossing && turn(state, runner, position)? {
        return Ok(true);
    }
    if !crossing && !terminal::at_bay(state, runner)? {
        set(state, id, false, "cur_speed", 0)?;
        return Ok(false);
    }
    if !terminal::enter(state, id, tile)?
        || (crossing && (32..48).contains(&track) && matches!(next_track, 6 | 7 | 14 | 15))
    {
        set(state, id, false, "cur_speed", 0)?;
        return Ok(false);
    }
    if crossing {
        terminal::leave(state, id)?;
        let _changed = turn(state, runner, position)?;
        set(state, id, true, "state", next_track)?;
    }
    set(state, id, false, "tile", i64::from(tile))?;
    set(state, id, true, "frame", next_frame)?;
    set(state, id, false, "x_pos", position.0)?;
    set(state, id, false, "y_pos", position.1)?;
    runner
        .spatial
        .update(SavedVehicleView::candidate(state.view(), id)?, state.width)?;
    Ok(true)
}
pub(super) fn advance(
    runner: &mut RoadPhases<'_>,
    state: &mut State<'_>,
) -> Result<(), WorldTickError> {
    let id = runner.id;
    routing::physical_tile(state, runner)?;
    let tile = SavedVehicleView::candidate(state.view(), id)?.tile()?;
    let local = frames::at(
        number(state, id, true, "state")?,
        number(state, id, true, "frame")?,
    )?;
    let width = i64::from(state.width);
    if number(state, id, false, "x_pos")?
        != (i64::from(tile) & width.wrapping_sub(1))
            .wrapping_mul(16)
            .wrapping_add(i64::from(local.0))
        || number(state, id, false, "y_pos")?
            != (i64::from(tile) >> width.trailing_zeros())
                .wrapping_mul(16)
                .wrapping_add(i64::from(local.1))
        || number(state, id, false, "z_pos")?
            != i64::from(state.view().tile(tile)?.height()).wrapping_mul(8)
    {
        return Err(unsupported("road_route", "physical drive-table position"));
    }
    let sub = number(state, id, false, "subspeed")?.wrapping_add(256);
    set(state, id, false, "subspeed", sub & 255)?;
    let speed = number(state, id, false, "cur_speed")?;
    let max = i64::from(runner.cache.max_track_speed)
        .min(number(state, id, false, "current_order.max_speed")?.wrapping_mul(2));
    let limit = if speed > max {
        speed.saturating_sub(speed / 10).saturating_sub(1).max(max)
    } else {
        max
    };
    let speed = speed.wrapping_add(sub >> 8).clamp(0, limit);
    set(state, id, false, "cur_speed", speed)?;
    let mut progress =
        (speed.wrapping_mul(3) / 4).wrapping_add(number(state, id, false, "progress")?);
    set(state, id, false, "progress", 0)?;
    while progress >= distance(state, runner)? {
        let distance = distance(state, runner)?;
        progress = progress.wrapping_sub(distance);
        if !step(state, runner)? {
            progress = distance.wrapping_sub(1);
            break;
        }
    }
    set(state, id, false, "progress", progress)?;
    runner.cache.last_speed = u16::try_from(number(state, id, false, "cur_speed")?)
        .map_err(|_| unsupported("road_route", "speed"))?;
    Ok(())
}
