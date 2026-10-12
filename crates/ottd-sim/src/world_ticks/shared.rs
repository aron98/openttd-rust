pub(crate) struct WorldTickState<'w> {
    transaction: ottd_save::world::WorldTransaction<'w>,
    width: u32,
    height: u32,
    towns: Option<crate::runtime::TownHouseCaches>,
    industries: crate::runtime::IndustryCaches,
}

#[derive(Debug, Clone)]
pub(crate) struct TileLoop {
    cursor: usize,
    feedback: usize,
    visits: usize,
}

pub(crate) trait WorldTickPhases {
    fn before_state_loop(&mut self, _: &mut WorldTickState<'_>) -> Result<(), WorldTickError> { Ok(()) }
    fn company_assets(&self, state: &WorldTickState<'_>, _:u32) -> Result<i64,WorldTickError> {
        if state.table(b"VEHS")?.records().next().is_some() || state.table(b"STNN")?.records().next().is_some() {
            return Err(unsupported_tick("company_assets","active assets require vehicle/station phase support"));
        }
        Ok(0)
    }
    fn station_tiles(&self) -> bool { false }
    fn transport_activity(&self) -> bool { false }
    fn cargo_activity(&self) -> bool { false }
    fn link_graphs(&mut self, _: &mut WorldTickState<'_>) -> Result<(), WorldTickError> { Ok(()) }
    fn station_month(&mut self, _: &mut WorldTickState<'_>) -> Result<(), WorldTickError> { Ok(()) }
    fn stations(&mut self, _: &mut WorldTickState<'_>) -> Result<(), WorldTickError> { Ok(()) }
    fn admit(&self, world: &ottd_save::world::World) -> Result<(), WorldTickError>;
    fn vehicle_year(&mut self, state: &mut WorldTickState<'_>, _: &ottd_core::ClockState) -> Result<(), WorldTickError> {
        if state.table(b"VEHS")?.records().next().is_some() {
            return Err(unsupported_tick("vehicle_year", "active vehicles require annual phase support"));
        }
        Ok(())
    }
    fn economy_boundary(&self, events: &ottd_core::ClockEvents) -> Result<(), WorldTickError>;
    fn calendar(
        &mut self,
        state: &mut WorldTickState<'_>,
        clock: &ottd_core::ClockState,
        progressed: bool,
    ) -> Result<(), WorldTickError>;
    fn ticks(
        &mut self,
        state: &mut WorldTickState<'_>,
        clock: &ottd_core::ClockState,
    ) -> Result<(), WorldTickError>;
    fn town_cargo(
        &mut self,
        state: &mut WorldTickState<'_>,
        production: world_ticks::house_cargo::TownCargoProduction,
    ) -> Result<u32, WorldTickError> {
        world_ticks::house_cargo::deliver_empty(state, production)
    }
    fn industry_cargo(
        &mut self,
        state: &mut WorldTickState<'_>,
        production: cargo::Production,
    ) -> Result<u32, WorldTickError> {
        world_ticks::industries::deliver_empty(state, production)
    }
}

pub(crate) fn unsupported_tick(phase: &'static str, reason: &str) -> WorldTickError {
    WorldTickError::Unsupported {
        phase,
        reason: reason.into(),
    }
}
