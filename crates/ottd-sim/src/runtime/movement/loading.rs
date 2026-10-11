//! Ordinary empty loading, preserving queue/payment/order phase boundaries.
mod admission;
#[cfg(test)]
mod failure_tests;
mod jobs;
pub(super) use admission::admit;
mod links;
mod load_cargo;
pub(super) use links::station_links;
mod orders;
pub(super) use jobs::tick as link_tick;
mod payment;
mod records;
use super::{
    RoadPhases, SavedVehicleView, State, VehicleId, WorldTickError, number, set, unsupported,
};
use ottd_save::WireValue;

pub(super) fn begin(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    station: u32,
) -> Result<(), WorldTickError> {
    let id = runner.id;
    if number(state, id, false, "current_order.type")? != 33
        || number(state, id, false, "current_order.dest")? != i64::from(station)
    {
        return Err(unsupported("road_loading", "unscheduled or implicit stop"));
    }
    let (_, real, _) = orders::cursor(state, id)?;
    set(
        state,
        id,
        true,
        "gv_flags",
        number(state, id, true, "gv_flags")? & !4,
    )?;
    set(
        state,
        id,
        false,
        "cur_implicit_order_index",
        i64::try_from(real).map_err(|_| unsupported("road_loading", "order index"))?,
    )?;
    set(state, id, false, "current_order.type", 3)?;
    orders::measure(state, id, true)?;
    set(state, id, false, "current_order.type", 195)?;
    links::arrival(state, runner, station)?;
    queue(state, station, true, runner.id)?;
    set(
        state,
        id,
        false,
        "vehicle_flags",
        number(state, id, false, "vehicle_flags")? & !1,
    )?;
    set(state, id, false, "load_unload_ticks", 1)?;
    payment::create(
        state,
        runner,
        u16::try_from(station).map_err(|_| unsupported("road_loading", "station ID"))?,
    )?;
    set(state, id, false, "cur_speed", 0)
}
fn queue(
    state: &mut State<'_>,
    station: u32,
    insert: bool,
    id: VehicleId,
) -> Result<(), WorldTickError> {
    let normal = records::normal(state, station)?;
    let value = crate::world_access::row_field(normal.schema, normal.record, "loading_vehicles")?;
    let WireValue::Array(values) = value else {
        return Err(unsupported("road_loading", "station queue"));
    };
    let pointer = WireValue::Unsigned(u64::from(id.raw()).wrapping_add(1));
    if insert && !values.is_empty() || !insert && values.as_slice() != [pointer.clone()] {
        return Err(unsupported("road_loading", "single vehicle station queue"));
    }
    records::station_field(
        state,
        station,
        (
            "loading_vehicles",
            WireValue::Array(if insert { vec![pointer] } else { Vec::new() }),
        ),
    )
}
pub(super) fn station_tick(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    let id = runner.id;
    if number(state, id, false, "current_order.type")? & 15 != 3 {
        return Ok(());
    }
    if number(state, id, false, "vehstatus")? & 0x82 != 0 {
        return Ok(());
    }
    let remaining = number(state, id, false, "load_unload_ticks")?
        .checked_sub(1)
        .ok_or_else(|| unsupported("road_loading", "loading timer underflow"))?;
    if remaining < 0 {
        return Err(unsupported("road_loading", "zero loading timer"));
    }
    set(state, id, false, "load_unload_ticks", remaining)?;
    if remaining != 0 {
        return Ok(());
    }
    let station = u32::try_from(number(state, id, false, "last_station_visited")?)
        .map_err(|_| unsupported("road_loading", "station ID"))?;
    if number(state, id, false, "vehicle_flags")? & 64 != 0 {
        payment::delete(state, runner)?;
        set(state, id, false, "load_unload_ticks", 20)?;
        return set(
            state,
            id,
            false,
            "vehicle_flags",
            (number(state, id, false, "vehicle_flags")? & !64) | 1,
        );
    }
    let cargo = SavedVehicleView::candidate(state.view(), id)?.cargo_type()?;
    let capacity = number(state, id, false, "cargo_cap")?;
    set(state, id, false, "refit_cap", capacity)?;
    set(state, id, false, "cur_speed", 0)?;
    let date = i32::try_from(state.number(b"DATE", 0, "date")?)
        .map_err(|_| unsupported("road_loading", "calendar date"))?;
    let year = ottd_core::CalendarDate::from_raw(date)
        .map_err(|_| unsupported("road_loading", "calendar date"))?
        .ymd()
        .0;
    let age = (i64::from(year).wrapping_sub(number(state, id, false, "build_year")?)).clamp(0, 255);
    for (name, value) in [
        ("last_speed", u64::from(runner.cache.max_speed / 2).min(255)),
        (
            "last_age",
            u64::try_from(age).map_err(|_| unsupported("road_loading", "vehicle age"))?,
        ),
    ] {
        records::goods_field(state, (station, cargo), (name, WireValue::Unsigned(value)))?;
    }
    let capacity = u32::try_from(capacity).map_err(|_| unsupported("cargo_load", "capacity"))?;
    let (loaded, possible) = load_cargo::load(state, runner, station, cargo, capacity)?;
    payment::delete(state, runner)?;
    let flags = number(state, id, false, "vehicle_flags")? & !64;
    if loaded != 0 {
        let wait = if state.number(b"PATS", 0, "order.gradual_loading")? != 0 {
            20
        } else {
            i64::from(loaded)
        };
        set(state, id, false, "load_unload_ticks", wait.max(1))?;
        set(
            state,
            id,
            false,
            "vehicle_flags",
            flags | if loaded < possible { 64 } else { 0 },
        )
    } else {
        set(state, id, false, "load_unload_ticks", 20)?;
        set(state, id, false, "vehicle_flags", flags | 1)
    }
}
pub(super) fn handle(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    if number(state, runner.id, false, "current_order.type")? & 15 != 3
        || number(state, runner.id, false, "vehicle_flags")? & 1 == 0
    {
        return Ok(());
    }
    payment::delete(state, runner)?;
    orders::measure(state, runner.id, false)?;
    let id = runner.id;
    set(
        state,
        id,
        false,
        "refit_cap",
        number(state, id, false, "cargo_cap")?,
    )?;
    links::refresh(state, runner)?;
    let station = number(state, id, false, "last_station_visited")?;
    set(state, id, false, "last_loading_station", station)?;
    set(
        state,
        id,
        false,
        "last_loading_tick",
        state.number(b"DATE", 0, "tick_counter")?,
    )?;
    set(state, id, false, "current_order.type", 4)?;
    set(state, id, false, "current_order.flags", 0)?;
    queue(
        state,
        u32::try_from(station).map_err(|_| unsupported("road_loading", "station ID"))?,
        false,
        id,
    )?;
    runner.cache.trip_occupancy = 0;
    orders::advance(state, id)
}
