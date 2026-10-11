mod access;
mod daily;
mod spatial;
mod stations;
mod step;
use super::{RoadVehicleCache, SavedVehicleView, SimulationRuntime, VehicleId};
use crate::{
    WorldTickError, WorldTickPhases as VehiclePhases, WorldTickReport, WorldTickState as State,
    content::{ContentCatalog, VehicleSpec},
    unsupported_tick as unsupported,
};
use access::{number, random, set};
use ottd_core::ClockState;
use ottd_save::{WireValue, world::World};
pub use spatial::SingleRoadTileOccupancy;
use spatial::Spatial;

impl SimulationRuntime {
    /// Advances the bounded single-road-vehicle gameplay domain atomically.
    /// Viewport sprite geometry is not part of this capability.
    /// # Errors
    /// Rejects unsupported initial or reached branches without changing saved state or caches.
    pub fn advance_world(&mut self, ticks: u32) -> Result<WorldTickReport, WorldTickError> {
        if ticks == 0 {
            return Ok(WorldTickReport { ticks: Vec::new() });
        }
        if self.road.len() != 1 {
            return Err(unsupported("road", "exactly one vehicle required"));
        }
        let (id, cache) = self
            .road
            .first_key_value()
            .ok_or_else(|| unsupported("road", "missing cache"))?;
        let id = *id;
        let mut runner = RoadPhases {
            id,
            cache: cache.clone(),
            content: &self.content,
            spatial: Spatial::restore(&self.world, id)?,
            stations: stations::IdleStations::restore(&self.world, &self.stations)?,
        };
        let (candidate, report) = State::plan(&mut self.world, ticks, &mut runner)?;
        let vehicle = SavedVehicleView::candidate(candidate.view(), id)?;
        runner.spatial.validate(vehicle, candidate.map().width())?;
        candidate.commit();
        self.road.insert(id, runner.cache);
        Ok(report)
    }
}
struct RoadPhases<'a> {
    id: VehicleId,
    cache: RoadVehicleCache,
    content: &'a ContentCatalog,
    spatial: Spatial,
    stations: stations::IdleStations,
}
impl VehiclePhases for RoadPhases<'_> {
    fn station_tiles(&self) -> bool {
        !self.stations.is_empty()
    }
    fn stations(&mut self, state: &mut State<'_>) -> Result<(), WorldTickError> {
        self.stations.tick(state)
    }
    fn admit(&self, world: &World) -> Result<(), WorldTickError> {
        if world
            .tables()
            .get(b"VEHS")
            .is_none_or(|t| t.records().len() != 1)
        {
            return Err(unsupported("road", "exactly one saved vehicle required"));
        }
        let v = SavedVehicleView::new(world, self.id)?;
        if !self.stations.is_empty() {
            return stations::admit_vehicle(v);
        }
        if v.setting("vehicle.roadveh_acceleration_model")? != 0
            || v.setting("vehicle.road_side")? != 1
            || v.setting("economy.timekeeping_units")? != 0
        {
            return Err(unsupported(
                "road",
                "original acceleration, right-hand calendar profile required",
            ));
        }
        for name in [
            "next_shared",
            "orders",
            "dest_tile",
            "vehicle_flags",
            "current_order.type",
            "current_order.flags",
            "breakdown_ctr",
            "load_unload_ticks",
            "waiting_triggers",
            "cur_real_order_index",
            "cur_implicit_order_index",
            "current_order.dest",
            "current_order.travel_time",
            "current_order.wait_time",
        ] {
            if v.common_number(name)? != 0 {
                return Err(unsupported("road", name));
            }
        }
        for (name, expected) in [
            ("direction", 1),
            ("vehstatus", 8),
            ("current_order.max_speed", 65535),
            ("current_order.refit_cargo", 254),
        ] {
            if v.common_number(name)? != expected {
                return Err(unsupported("road", name));
            }
        }
        for name in [
            "blocked_ctr",
            "overtaking",
            "overtaking_ctr",
            "reverse_ctr",
            "crashed_ctr",
            "state",
            "gv_flags",
        ] {
            if !matches!(v.road_field(name)?, WireValue::Unsigned(0)) {
                return Err(unsupported("road", name));
            }
        }
        if !matches!(v.road_field("path")?, WireValue::Structs(rows) if rows.is_empty())
            || v.stored_count()? != 0
        {
            return Err(unsupported("road", "saved path or cargo"));
        }
        if v.current_speed()? == 0 {
            return Err(unsupported("road", "already-moving vehicle required"));
        }
        if self.cache.visual_effect != 64 || self.cache.vehicle_length != 8 {
            return Err(unsupported("road", "effect or length"));
        }
        if !matches!(v.engine_id()?, 116 | 123) {
            return Err(unsupported("road", "native-covered engine capability"));
        }
        let engine = self
            .content
            .engines()
            .get(usize::from(v.engine_id()?))
            .ok_or_else(|| unsupported("road", "engine"))?;
        if !matches!(engine.vehicle, VehicleSpec::Road(_)) {
            return Err(unsupported("road", "engine type"));
        }
        Ok(())
    }
    fn economy_boundary(&self, events: &ottd_core::ClockEvents) -> Result<(), WorldTickError> {
        if events.iter().any(|event| {
            matches!(
                event,
                ottd_core::ClockEvent::EconomyMonth | ottd_core::ClockEvent::EconomyYear
            )
        }) {
            return Err(unsupported("road_economy", "vehicle month/year callbacks"));
        }
        Ok(())
    }
    fn calendar(
        &mut self,
        state: &mut State<'_>,
        clock: &ClockState,
        progressed: bool,
    ) -> Result<(), WorldTickError> {
        if progressed && clock.snapshot().date_fract.0 == 0 && clock.snapshot().date.ymd().2 == 1 {
            return Err(unsupported("road_calendar", "vehicle month/year callbacks"));
        }
        if progressed && self.id.raw() % 74 == u32::from(clock.snapshot().date_fract.0) {
            let age = number(state, self.id, false, "age")?.saturating_add(1);
            if age >= number(state, self.id, false, "max_age")?.saturating_sub(366) {
                return Err(unsupported("road_calendar", "old vehicle lifecycle"));
            }
            set(state, self.id, false, "age", age)?;
        }
        Ok(())
    }
    fn ticks(&mut self, state: &mut State<'_>, clock: &ClockState) -> Result<(), WorldTickError> {
        if self.id.raw() % 74 == u32::from(clock.snapshot().economy_date_fract.0) {
            daily::run(state, self.id, self.content)?;
        }
        if self.stations.is_empty() {
            self.tick(state)
        } else {
            stations::vehicle_tick(state, self.id, &mut self.cache)
        }
    }
}
