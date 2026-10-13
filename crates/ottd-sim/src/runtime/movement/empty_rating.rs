//! Empty-runtime candidate ownership of the unsaved company rating cache.
use super::super::CompanyRatingCaches;
use crate::{WorldTickError, WorldTickPhases, WorldTickState, world_ticks::EmptyVehicles};
use ottd_core::{ClockEvents, ClockState};
use ottd_save::world::World;

pub(super) struct EmptyRatingPhases {
    pub(super) ratings: CompanyRatingCaches,
}
impl WorldTickPhases for EmptyRatingPhases {
    fn company_rating(&mut self, owner: u32, parts: [i64; 10]) -> Result<(), WorldTickError> {
        Ok(self.ratings.update(owner, parts)?)
    }
    fn admit(&self, world: &World) -> Result<(), WorldTickError> {
        EmptyVehicles.admit(world)
    }
    fn economy_boundary(&self, events: &ClockEvents) -> Result<(), WorldTickError> {
        EmptyVehicles.economy_boundary(events)
    }
    fn calendar(
        &mut self,
        state: &mut WorldTickState<'_>,
        clock: &ClockState,
        progressed: bool,
    ) -> Result<(), WorldTickError> {
        EmptyVehicles.calendar(state, clock, progressed)
    }
    fn ticks(
        &mut self,
        state: &mut WorldTickState<'_>,
        clock: &ClockState,
    ) -> Result<(), WorldTickError> {
        EmptyVehicles.ticks(state, clock)
    }
}
