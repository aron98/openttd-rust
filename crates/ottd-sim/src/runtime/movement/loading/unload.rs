//! Native single ordinary road unloading slice before any loading branch.
use super::{
    RoadPhases, SavedVehicleView, State, WorldTickError, delivery::Delivery, number, records, set,
    unsupported,
};
use crate::runtime::transport::cargo::unload::Unload;
use ottd_save::WireValue;

pub(super) fn tick(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    station: u32,
) -> Result<bool, WorldTickError> {
    let flags = number(state, runner.id, false, "vehicle_flags")?;
    if flags & 2 == 0 {
        return Ok(false);
    }
    set(state, runner.id, false, "cur_speed", 0)?;
    let vehicle = SavedVehicleView::candidate(state.view(), runner.id)?;
    let cargo = vehicle.cargo_type()?;
    let owner = u32::from(vehicle.owner()?);
    let engine = usize::from(vehicle.engine_id()?);
    let gradual = state.number(b"PATS", 0, "order.gradual_loading")? != 0;
    let list = runner
        .transport
        .cargo
        .vehicles
        .get(&runner.id.raw())
        .ok_or_else(|| unsupported("cargo_unload", "vehicle cargo"))?;
    let mut amount = list.actions[0].wrapping_add(list.actions[1]);
    if gradual {
        amount = amount.min(
            u32::from(
                runner
                    .content
                    .engines()
                    .get(engine)
                    .ok_or_else(|| unsupported("cargo_unload", "engine"))?
                    .info
                    .load_amount,
            )
            .max(1),
        );
    }
    let acceptance_lost =
        records::goods(state, station, cargo)?.number("status")? & 1 == 0 && list.actions[1] != 0;
    if acceptance_lost {
        runner.transport.cargo.keep_delivery(state, runner.id)?;
    }
    let payment = *runner
        .transport
        .payment_stations
        .first_key_value()
        .ok_or_else(|| unsupported("cargo_unload", "payment"))?
        .0;
    let tile = u32::try_from(number(state, runner.id, false, "tile")?)
        .map_err(|_| unsupported("cargo_unload", "tile"))?;
    let spec = runner
        .content
        .cargo()
        .get(usize::from(cargo))
        .ok_or_else(|| unsupported("cargo_unload", "cargo specification"))?;
    runner
        .transport
        .cargo
        .stations
        .get_mut(&station)
        .ok_or_else(|| unsupported("cargo_unload", "station cargo"))?
        .entry(u32::from(cargo))
        .or_default();
    let delivery = Delivery {
        station,
        owner,
        cargo,
        tile,
        payment,
        spec,
    };
    let unloaded = runner.transport.cargo.deliver(
        state,
        Unload {
            vehicle: runner.id,
            amount,
        },
        |state, packet, count| delivery.pay(state, packet, count),
    )?;

    if unloaded == 0 && !acceptance_lost {
        return Err(unsupported("cargo_unload", "empty unloading slice"));
    }
    finish(state, runner, unloaded)
}

fn finish(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    unloaded: u32,
) -> Result<bool, WorldTickError> {
    let station = u32::try_from(number(state, runner.id, false, "last_station_visited")?)
        .map_err(|_| unsupported("cargo_unload", "station id"))?;
    let flags = number(state, runner.id, false, "vehicle_flags")?;
    let gradual = state.number(b"PATS", 0, "order.gradual_loading")? != 0;
    if unloaded > 0 {
        records::station_field(
            state,
            station,
            ("time_since_unload", WireValue::Unsigned(0)),
        )?;
    }
    let list = runner
        .transport
        .cargo
        .vehicles
        .get(&runner.id.raw())
        .ok_or_else(|| unsupported("cargo_unload", "vehicle cargo"))?;
    let complete = !gradual || list.actions[0].wrapping_add(list.actions[1]) == 0;
    set(
        state,
        runner.id,
        false,
        "vehicle_flags",
        flags & !64 & if complete { !2 } else { -1 },
    )?;
    set(
        state,
        runner.id,
        false,
        "load_unload_ticks",
        if gradual {
            20
        } else {
            i64::from(unloaded).max(1)
        },
    )?;
    if complete {
        set(
            state,
            runner.id,
            false,
            "waiting_triggers",
            number(state, runner.id, false, "waiting_triggers")? | 4,
        )?;
        super::super::random(state)?;
    }
    crate::runtime::road_cache::cargo_changed(
        &mut runner.cache,
        SavedVehicleView::candidate(state.view(), runner.id)?,
        runner.content,
    )?;
    Ok(true)
}
