//! Native terminal entrance allocation and bay lifecycle, separate from frame motion.
use super::{
    RoadPhases, SavedVehicleView, State, VehicleId, WorldTickError, number, routing, set,
    unsupported,
};
use super::{terminal_far as far, terminal_near as near};
pub(super) fn frame(track: i64, frame: i64) -> Result<(u8, u8), WorldTickError> {
    let data = match track & !4 {
        32 => far::SW,
        33 => far::NW,
        34 => near::SW,
        35 => near::NW,
        40 => far::NE,
        41 => far::SE,
        42 => near::NE,
        43 => near::SE,
        _ => return Err(unsupported("road_terminal", "terminal drive state")),
    };
    data.get(usize::try_from(frame).map_err(|_| unsupported("road_terminal", "frame"))?)
        .copied()
        .ok_or_else(|| unsupported("road_terminal", "terminal frame"))
}
pub(super) fn enter(
    state: &mut State<'_>,
    id: VehicleId,
    tile: u32,
) -> Result<bool, WorldTickError> {
    if state.view().tile(tile)?.tile_type() >> 4 != 5 {
        return Ok(true);
    }
    let track = number(state, id, true, "state")?;
    if track >= 32 || matches!(track, 6 | 7 | 14 | 15) || number(state, id, true, "frame")? != 0 {
        return Ok(true);
    }
    let (stop, status) = routing::stop(state, id, tile)?;
    if status & 128 != 0 || status.trailing_zeros() >= 2 {
        return Ok(false);
    }
    let bay = i64::from(status & 1 == 0);
    state.set_number(b"ROAD", stop, "status", (status & !(1 << bay)) | 128)?;
    set(state, id, true, "state", ((track | 32) & !2) | (bay << 1))?;
    Ok(true)
}
pub(super) fn leave(state: &mut State<'_>, id: VehicleId) -> Result<(), WorldTickError> {
    let track = number(state, id, true, "state")?;
    if !(32..48).contains(&track) {
        return Ok(());
    }
    let tile = SavedVehicleView::candidate(state.view(), id)?.tile()?;
    let (stop, status) = routing::stop(state, id, tile)?;
    state.set_number(
        b"ROAD",
        stop,
        "status",
        (status | (1 << ((track >> 1) & 1))) & !128,
    )
}
pub(super) fn at_bay(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<bool, WorldTickError> {
    let id = runner.id;
    let track = number(state, id, true, "state")?;
    if !(32..48).contains(&track) {
        return Ok(true);
    }
    let stop_frame = match track & !4 {
        32 | 33 => 16,
        34 | 35 => 12,
        40 | 41 => 15,
        42 | 43 => 11,
        _ => return Err(unsupported("road_terminal", "bay frame")),
    };
    if number(state, id, true, "frame")? != stop_frame {
        return Ok(true);
    }
    let tile = SavedVehicleView::candidate(state.view(), id)?.tile()?;
    let (stop, status) = routing::stop(state, id, tile)?;
    if track & 4 == 0 {
        state.set_number(b"ROAD", stop, "status", status & !128)?;
        set(state, id, true, "state", track | 4)?;
        let station = u32::from(state.view().tile(tile)?.m2());
        set(state, id, false, "last_station_visited", i64::from(station))?;
        let passenger = runner
            .content
            .cargo()
            .get(usize::from(
                SavedVehicleView::candidate(state.view(), id)?.cargo_type()?,
            ))
            .ok_or_else(|| unsupported("road_terminal", "cargo specification"))?
            .classes
            & 1
            != 0;
        let table = state.table(b"STNN")?;
        let row = crate::runtime::stations::saved::StationRow {
            schema: table.schema(),
            record: table
                .record(station)
                .ok_or_else(|| unsupported("road_terminal", "station"))?,
        };
        let seen =
            row.child("normal")?.number("had_vehicle_of_type")? | if passenger { 4 } else { 8 };
        state.apply(ottd_save::world::WorldEdit::Field {
            chunk: *b"STNN",
            record: station,
            path: vec![
                ottd_save::world::PathElement::Field("normal".into()),
                ottd_save::world::PathElement::Index(0),
                ottd_save::world::PathElement::Field("had_vehicle_of_type".into()),
            ],
            value: ottd_save::WireValue::Unsigned(seen),
        })?;
        super::super::loading::begin(state, runner, station)?;
        return Ok(false);
    }
    if status & 128 != 0 {
        return Ok(false);
    }
    if number(state, id, false, "current_order.type")? & 15 == 4 {
        for field in [
            "current_order.type",
            "current_order.flags",
            "current_order.dest",
        ] {
            set(state, id, false, field, 0)?;
        }
    }
    state.set_number(b"ROAD", stop, "status", status | 128)?;
    Ok(true)
}
