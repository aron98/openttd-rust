//! Synthetic immediate-hook fault; distinct from the original-backed late boundary.
use super::super::{EmptyVehicles, unsupported};
use crate::{
    WorldTickError, WorldTickPhases, WorldTickState as State,
    runtime::{CompanyRatingCaches, IndustryCaches},
};
use ottd_core::{ClockEvents, ClockState};
use ottd_save::{Savegame, world::World};

struct RatingFault {
    ratings: CompanyRatingCaches,
}
impl WorldTickPhases for RatingFault {
    fn company_rating(&mut self, owner: u32, parts: [i64; 10]) -> Result<(), WorldTickError> {
        self.ratings.update(owner, parts)?;
        Err(unsupported(
            "synthetic_rating_fault",
            "after candidate score publication",
        ))
    }
    fn admit(&self, world: &World) -> Result<(), WorldTickError> {
        EmptyVehicles.admit(world)
    }
    fn economy_boundary(&self, events: &ClockEvents) -> Result<(), WorldTickError> {
        EmptyVehicles.economy_boundary(events)
    }
    fn calendar(
        &mut self,
        state: &mut State<'_>,
        clock: &ClockState,
        progressed: bool,
    ) -> Result<(), WorldTickError> {
        EmptyVehicles.calendar(state, clock, progressed)
    }
    fn ticks(&mut self, state: &mut State<'_>, clock: &ClockState) -> Result<(), WorldTickError> {
        EmptyVehicles.ticks(state, clock)
    }
}
#[test]
fn synthetic_failure_after_rating_discards_world_and_candidate_cache_publication()
-> Result<(), Box<dyn std::error::Error>> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let committed = CompanyRatingCaches::fresh_process();
    let mut candidate = RatingFault {
        ratings: committed.clone(),
    };
    let before = world.saved_json()?;
    let result = {
        let industries = IndustryCaches::restore(&world)?;
        let mut state = State::load(&mut world, None, industries);
        super::quarter(&mut state, 0, &mut candidate)
    };
    assert!(matches!(
        result,
        Err(WorldTickError::Unsupported {
            phase: "synthetic_rating_fault",
            ..
        })
    ));
    assert_ne!(candidate.ratings, committed);
    assert_eq!(committed.parts(), &[[0; 10]; 15]);
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
