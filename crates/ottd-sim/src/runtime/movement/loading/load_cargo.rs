//! Ordinary road loading, including native trigger draws and gradual slice timing.
use super::{RoadPhases, State, WorldTickError, number, orders, records, set, unsupported};
use crate::runtime::transport::cargo::load::Load;
use ottd_save::WireValue;

pub(super) fn load(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    station: u32,
    cargo: u8,
    capacity: u32,
) -> Result<(u32, u32), WorldTickError> {
    let stored = runner
        .transport
        .cargo
        .vehicles
        .get(&runner.id.raw())
        .ok_or_else(|| unsupported("cargo_load", "vehicle list"))?
        .count;
    let mut left = capacity
        .checked_sub(stored)
        .ok_or_else(|| unsupported("cargo_load", "over capacity"))?;
    if left == 0 {
        return Ok((0, 0));
    }
    records::goods_field(
        state,
        (station, cargo),
        ("time_since_pickup", WireValue::Unsigned(0)),
    )?;
    let available = runner
        .transport
        .cargo
        .stations
        .get(&station)
        .and_then(|goods| goods.get(&u32::from(cargo)))
        .map_or(0, |list| list.count);
    let town = records::normal(state, station)?
        .child("base")?
        .number("town")?
        .checked_sub(1)
        .ok_or_else(|| unsupported("cargo_load", "station town"))?;
    let town = u32::try_from(town).map_err(|_| unsupported("cargo_load", "town id"))?;
    if available == 0
        || (state.number(b"CITY", town, "exclusive_counter")? != 0
            && state.number(b"CITY", town, "exclusivity")?
                != number(state, runner.id, false, "owner")?)
    {
        return Ok((0, left));
    }
    if stored == 0 {
        set(
            state,
            runner.id,
            false,
            "waiting_triggers",
            number(state, runner.id, false, "waiting_triggers")? | 9,
        )?;
        super::super::random(state)?;
        super::super::random(state)?;
    }
    if state.number(b"PATS", 0, "order.gradual_loading")? != 0 {
        let engine = usize::try_from(number(state, runner.id, false, "engine_type")?)
            .map_err(|_| unsupported("cargo_load", "engine id"))?;
        let engine = runner
            .content
            .engines()
            .get(engine)
            .ok_or_else(|| unsupported("cargo_load", "engine"))?;
        left = left.min(u32::from(engine.info.load_amount).max(1));
    }
    let destinations = orders::destinations(state, runner.id)?;
    let (_, current, len) = orders::cursor(state, runner.id)?;
    let len =
        std::num::NonZeroUsize::new(len).ok_or_else(|| unsupported("cargo_load", "orders"))?;
    let next = *destinations
        .get(current.wrapping_add(1) % len)
        .ok_or_else(|| unsupported("cargo_load", "next station"))?;
    let next = u16::try_from(next).map_err(|_| unsupported("cargo_load", "next station id"))?;
    let tile = u32::try_from(number(state, runner.id, false, "tile")?)
        .map_err(|_| unsupported("cargo_load", "tile"))?;
    let moved = runner.transport.cargo.load(
        state,
        Load {
            station,
            cargo,
            vehicle: runner.id,
            amount: left,
            tile,
            next: &[next],
        },
    )?;
    if moved != 0 {
        records::station_field(state, station, ("time_since_load", WireValue::Unsigned(0)))?;
        records::station_field(
            state,
            station,
            ("last_vehicle_type", WireValue::Unsigned(1)),
        )?;
    }
    Ok((moved, left))
}
