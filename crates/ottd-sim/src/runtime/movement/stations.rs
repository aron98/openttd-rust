//! Native terminal station rating, acceptance and monthly cargo history phases.
use super::{
    RoadVehicleCache, SavedVehicleView, State, VehicleId, WorldTickError, number, set, unsupported,
};
use crate::runtime::stations::{StationRuntime, saved};
use crate::world_access::row_field;
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};

pub(super) struct IdleStations(Vec<u32>);
impl IdleStations {
    pub(super) fn restore(world: &World, runtime: &StationRuntime) -> Result<Self, WorldTickError> {
        let pause = crate::world_access::unsigned(world, b"DATE", 0, "pause_mode")?;
        if pause != 0 && pause != 64 && pause != 192 {
            return Ok(Self(Vec::new()));
        }
        let table = world
            .tables()
            .get(b"STNN")
            .ok_or_else(|| unsupported("station", "missing pool"))?;
        let mut ids = Vec::new();
        for id in table.records().keys() {
            let row = saved::row(world, *b"STNN", *id)?;
            let facilities = row.number("facilities")?;
            if facilities == 0 || facilities & !6 != 0 || !runtime.geometry.contains_key(id) {
                return Err(unsupported("station", "active terminal stations required"));
            }
            for name in ["speclist", "roadstopspeclist", "roadstoptiledata"] {
                if !row.rows(name)?.1.is_empty() {
                    return Err(unsupported("station", "custom stop callbacks"));
                }
            }
            let normal = row.child("normal")?;
            let base = normal.child("base")?;
            if base.number("delete_ctr")? >= 185 || base.number("waiting_triggers")? != 0 {
                return Err(unsupported("station", "idle counter or trigger"));
            }
            if !matches!(normal.number("last_vehicle_type")?, 1 | 255)
                || normal.number("had_vehicle_of_type")? & !0x0c != 0
                || !matches!(row_field(normal.schema, normal.record, "loading_vehicles")?, WireValue::Array(values) if values.len() <= 1)
            {
                return Err(unsupported("station", "non-road loading activity"));
            }
            let (schema, goods) = normal.rows("goods")?;
            if goods.len() != 64 {
                return Err(unsupported("station", "goods cardinality"));
            }
            for record in goods {
                let cargo = saved::StationRow { schema, record };
                if !cargo.rows("flow")?.1.is_empty() {
                    return Err(unsupported("station", "automatic cargo flow data"));
                }
            }
            ids.push(*id);
        }
        for tile in world.map().tiles() {
            if tile.tile_type() >> 4 == 5
                && (!matches!((tile.m6() >> 3) & 15, 2 | 3) || tile.m5() >= 4)
            {
                return Err(unsupported("station", "non-terminal station tile"));
            }
        }
        Ok(Self(ids))
    }
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(super) fn month(&self, state: &mut State<'_>) -> Result<(), WorldTickError> {
        for id in &self.0 {
            for cargo in 0..64 {
                let status = goods_status(state, *id, cargo)?;
                goods_status_set(state, *id, cargo, (status & !0x18) | ((status & 0x10) >> 1))?;
            }
        }
        Ok(())
    }
    pub(super) fn tick(
        &self,
        state: &mut State<'_>,
        transport: &mut crate::runtime::transport::TransportRuntime,
        stations: &StationRuntime,
        content: &crate::content::ContentCatalog,
    ) -> Result<(), WorldTickError> {
        transport.cargo.refresh_recipients(state, stations)?;
        for id in &self.0 {
            let table = state.table(b"STNN")?;
            let row = saved::StationRow {
                schema: table.schema(),
                record: table
                    .record(*id)
                    .ok_or_else(|| unsupported("station", "missing row"))?,
            };
            let counter = row.child("normal")?.child("base")?.number("delete_ctr")?;
            state.apply(WorldEdit::Field {
                chunk: *b"STNN",
                record: *id,
                path: vec![
                    PathElement::Field("normal".into()),
                    PathElement::Index(0),
                    PathElement::Field("base".into()),
                    PathElement::Index(0),
                    PathElement::Field("delete_ctr".into()),
                ],
                value: WireValue::Unsigned(counter.wrapping_add(1) % 185),
            })?;
            if counter == 184 {
                transport.cargo.rate_station(state, *id, content)?;
            }
            super::loading::station_links(state, *id)?;
            if state
                .number(b"DATE", 0, "tick_counter")?
                .wrapping_add(i64::from(*id))
                % 250
                == 0
            {
                for cargo in 0..64 {
                    let status = goods_status(state, *id, cargo)?;
                    goods_status_set(state, *id, cargo, status & !32)?;
                }
                let geometry = stations
                    .geometry
                    .get(id)
                    .ok_or_else(|| unsupported("station_acceptance", "geometry"))?;
                let view = crate::runtime::transport::cargo::read::Read::Candidate(state.view());
                for edit in crate::runtime::stations::acceptance::calculate(view, geometry)?
                    .edits(view, *id, true)?
                {
                    state.apply(edit)?;
                }
            }
        }
        Ok(())
    }
}

fn goods_status(state: &State<'_>, station: u32, cargo: usize) -> Result<u64, WorldTickError> {
    let row = crate::runtime::transport::cargo::read::Read::Candidate(state.view())
        .row(*b"STNN", station)?
        .child("normal")?;
    let (schema, rows) = row.rows("goods")?;
    Ok(saved::StationRow {
        schema,
        record: rows
            .get(cargo)
            .ok_or_else(|| unsupported("station", "goods"))?,
    }
    .number("status")?)
}
fn goods_status_set(
    state: &mut State<'_>,
    station: u32,
    cargo: usize,
    value: u64,
) -> Result<(), WorldTickError> {
    state.apply(WorldEdit::Field {
        chunk: *b"STNN",
        record: station,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field("goods".into()),
            PathElement::Index(cargo),
            PathElement::Field("status".into()),
        ],
        value: WireValue::Unsigned(value),
    })
}

pub(super) fn admit_vehicle(v: SavedVehicleView<'_>) -> Result<(), WorldTickError> {
    if !matches!(v.engine_id()?, 116 | 123)
        || v.stored_count()? != 0
        || v.common_number("vehstatus")? != 11
        || v.current_speed()? != 0
        || !matches!(v.road_field("state")?, WireValue::Unsigned(254))
        || v.setting("vehicle.roadveh_acceleration_model")? != 0
        || v.setting("vehicle.road_side")? != 1
        || v.setting("economy.timekeeping_units")? != 0
    {
        return Err(unsupported(
            "road_route",
            "station phase currently requires a stopped depot vehicle",
        ));
    }
    for name in [
        "breakdown_ctr",
        "current_order.type",
        "vehicle_flags",
        "load_unload_ticks",
    ] {
        if v.common_number(name)? != 0 {
            return Err(unsupported("road_route", name));
        }
    }
    Ok(())
}

pub(super) fn vehicle_tick(
    state: &mut State<'_>,
    id: VehicleId,
    cache: &mut RoadVehicleCache,
) -> Result<(), WorldTickError> {
    set(
        state,
        id,
        false,
        "tick_counter",
        number(state, id, false, "tick_counter")?.wrapping_add(1) & 255,
    )?;
    set(
        state,
        id,
        false,
        "current_order_time",
        number(state, id, false, "current_order_time")?.wrapping_add(1) & 0xffff_ffff,
    )?;
    let reverse = number(state, id, true, "reverse_ctr")?;
    if reverse != 0 {
        set(state, id, true, "reverse_ctr", reverse.wrapping_sub(1))?;
    }
    cache.last_speed = 0;
    age_cargo(state, id, cache)
}
pub(super) fn age_cargo(
    state: &mut State<'_>,
    id: VehicleId,
    cache: &RoadVehicleCache,
) -> Result<(), WorldTickError> {
    if cache.cargo_age_period != 0 {
        let period = i64::from(cache.cargo_age_period);
        let next = number(state, id, false, "cargo_age_counter")?
            .min(period)
            .wrapping_sub(1)
            & 65535;
        set(
            state,
            id,
            false,
            "cargo_age_counter",
            if next == 0 { period } else { next },
        )?;
    }
    Ok(())
}
