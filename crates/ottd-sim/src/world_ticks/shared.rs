pub(crate) struct WorldTickState<'w> {
    transaction: ottd_save::world::WorldTransaction<'w>,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct TileLoop {
    cursor: usize,
    feedback: usize,
    visits: usize,
}

pub(crate) trait WorldTickPhases {
    fn company_assets(&self, state: &WorldTickState<'_>, _:u32) -> Result<i64,WorldTickError> {
        if state.table(b"VEHS")?.records().next().is_some() || state.table(b"STNN")?.records().next().is_some() {
            return Err(unsupported_tick("company_assets","active assets require vehicle/station phase support"));
        }
        Ok(0)
    }
    fn station_tiles(&self) -> bool { false }
    fn stations(&mut self, _: &mut WorldTickState<'_>) -> Result<(), WorldTickError> { Ok(()) }
    fn admit(&self, world: &ottd_save::world::World) -> Result<(), WorldTickError>;
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
}

pub(crate) fn unsupported_tick(phase: &'static str, reason: &str) -> WorldTickError {
    WorldTickError::Unsupported {
        phase,
        reason: reason.into(),
    }
}
