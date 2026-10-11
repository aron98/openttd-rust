//! Atomic single-road world advancement and candidate publication.
use super::{
    RoadPhases, SavedVehicleView, SimulationRuntime, State, WorldTickError, WorldTickReport,
    spatial, stations, unsupported,
};

impl SimulationRuntime {
    /// Advances the bounded single-road-vehicle gameplay domain atomically.
    /// Viewport sprite geometry is not part of this capability.
    /// # Errors
    /// Rejects unsupported initial or reached branches without changing saved state or caches.
    pub fn advance_world(&mut self, ticks: u32) -> Result<WorldTickReport, WorldTickError> {
        #[cfg(feature = "async-trace")]
        let mut trace = crate::runtime::async_trace::Candidate::begin(&self.transport);
        if ticks == 0 {
            #[cfg(feature = "async-trace")]
            trace.commit(&self.transport);
            return Ok(WorldTickReport { ticks: Vec::new() });
        }
        if self.save_context() != super::RuntimeSaveContext::SinglePlayer {
            return Err(unsupported("host", "network world-loop context"));
        }
        if self.road.is_empty() {
            let (candidate, report, towns) = State::plan(
                &mut self.world,
                ticks,
                &mut crate::world_ticks::EmptyVehicles,
            )?;
            #[cfg(feature = "async-trace")]
            trace.prepared();
            candidate.commit();
            if let Some(towns) = towns {
                self.towns = towns;
            }
            #[cfg(feature = "async-trace")]
            trace.commit(&self.transport);
            return Ok(report);
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
            spatial: spatial::Spatial::restore(&self.world, id)?,
            stations: stations::IdleStations::restore(&self.world, &self.stations)?,
            station_context: &self.stations,
            transport: self.transport.clone(),
        };
        let (candidate, report, towns) = State::plan(&mut self.world, ticks, &mut runner)?;
        let vehicle = SavedVehicleView::candidate(candidate.view(), id)?;
        runner.spatial.validate(vehicle, candidate.map().width())?;
        runner.transport.validate(candidate.view())?;
        #[cfg(feature = "async-trace")]
        trace.prepared();
        candidate.commit();
        self.road.insert(id, runner.cache);
        self.transport = runner.transport;
        if let Some(towns) = towns {
            self.towns = towns;
        }
        #[cfg(feature = "async-trace")]
        trace.commit(&self.transport);
        Ok(report)
    }
}
