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
