use crate::{Fixture, Landscape, LandscapeError, TickEvents, clock};

/// Maximum steps per request, bounding CPU work and trace allocation.
pub const MAX_TICKS: u32 = 1_000_000;

/// Rejection at the subsystem input boundary.
#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    /// An unsupported context or schema version was requested.
    #[error(
        "expected schema 1, temperate normal gameplay, no ambient callbacks, and timekeeping units 0 or 1"
    )]
    Context,
    /// A calendar date was out of range.
    #[error("calendar date outside supported years 0..=5000000")]
    CalendarDate,
    /// The request exceeds the deterministic execution budget.
    #[error("requested ticks exceed the per-request limit of {MAX_TICKS}")]
    TickLimit,
    /// Terrain or scheduler validation failed.
    #[error(transparent)]
    Landscape(#[from] LandscapeError),
    /// Clock validation failed.
    #[error(transparent)]
    Clock(#[from] ottd_core::ClockError),
}

/// Advances only clocks and supported clear terrain, preserving gameplay RNG.
///
/// Clock boundary callbacks are reported but their bodies are not executed.
/// Prior input events are discarded; the result traces only these requested calls.
/// # Errors
/// Rejects unsupported context, malformed clocks/terrain, or excessive tick counts
/// before any state advances.
pub fn simulate(mut fixture: Fixture, ticks: u32) -> Result<Fixture, SimulationError> {
    if ticks > MAX_TICKS {
        return Err(SimulationError::TickLimit);
    }
    if fixture.schema_version != 1
        || fixture.context.landscape != "temperate"
        || fixture.context.mode != "normal"
        || fixture.context.ambient_callbacks
    {
        return Err(SimulationError::Context);
    }
    let mut clock = clock::restore(&fixture.state.clock, &fixture.context)?;
    let mut landscape = Landscape::new(fixture.state.map, fixture.state.cur_tileloop_tile)?;
    fixture.events.clear();
    for _ in 0..ticks {
        let events = clock.advance(fixture.context.paused);
        let tick_counter = clock.snapshot().tick_counter.0;
        if !fixture.context.paused {
            landscape.advance(tick_counter)?;
        }
        fixture.events.push(TickEvents {
            tick_counter,
            calendar_progressed: events.calendar_progressed,
            events: events.iter().map(|event| event.name().to_owned()).collect(),
        });
    }
    fixture.state.clock = clock.into();
    fixture.state.cur_tileloop_tile =
        u32::try_from(landscape.cursor()).map_err(|_| LandscapeError::Cursor)?;
    fixture.state.map = landscape.into_map();
    Ok(fixture)
}
