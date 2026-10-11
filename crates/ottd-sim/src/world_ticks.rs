//! Transactional native state-loop execution for explicitly admitted saved worlds.
mod access;
mod clocks;
mod company_history;
mod domain;
mod finance;
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed producer handoff is shared internally with station transactions"
)]
pub(crate) mod house_cargo;
mod houses;
mod phases;
use clocks::{restore_clock, save_clock};
use phases::{competitor, daily, industry_month};
mod tiles;
mod towns;
mod trees;
use crate::{
    Landscape, MAX_TICKS, TickEvents,
    world_access::{WorldAccessError, unsigned},
};
use crate::{
    WorldTickPhases as VehiclePhases, WorldTickState as State, unsupported_tick as unsupported,
};
use ottd_core::{ClockEvent, ClockEvents, ClockPhase, ClockState};
use ottd_save::{
    WireValue,
    world::{PreparedWorldTransaction, World, WorldEdit},
};
use serde::{Deserialize, Serialize};
use tiles::{map, tile};

/// Rejected unsupported phase or invalid saved-world operation.
#[derive(Debug, thiserror::Error)]
pub enum WorldTickError {
    /// Native work outside the implemented world domain.
    #[error("unsupported world tick phase {phase}: {reason}")]
    Unsupported {
        /// Native phase.
        phase: &'static str,
        /// Rejected saved context.
        reason: String,
    },
    /// Saved field access failure.
    #[error(transparent)]
    Access(#[from] WorldAccessError),
    /// Final saved-world validation failure.
    #[error(transparent)]
    World(#[from] ottd_save::world::WorldError),
    /// Runtime vehicle admission or restoration failed.
    #[error(transparent)]
    Runtime(#[from] crate::runtime::RuntimeError),
    /// Clock validation failure.
    #[error(transparent)]
    Clock(#[from] ottd_core::ClockError),
    /// Tile-loop domain validation failure.
    #[error(transparent)]
    Landscape(#[from] crate::LandscapeError),
    /// Existing industry bookkeeping callback failure.
    #[error(transparent)]
    Industry(#[from] crate::IndustryCallbackError),
    /// Existing company bookkeeping callback failure.
    #[error(transparent)]
    Company(#[from] crate::PeriodicCallbackError),
}

/// Ordered clock-boundary observations; authoritative results remain in the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldTickReport {
    /// One entry per requested state-loop call, including paused calls.
    pub ticks: Vec<TickEvents>,
}

/// Execute complete admitted state-loop calls and atomically publish their saved effects.
/// # Errors
/// Rejects unsupported state or any event reached within the horizon without changing `world`.
pub fn advance_world(world: &mut World, ticks: u32) -> Result<WorldTickReport, WorldTickError> {
    if ticks == 0 {
        return Ok(WorldTickReport { ticks: Vec::new() });
    }
    let (candidate, report, _) = State::plan(world, ticks, &mut EmptyVehicles)?;
    candidate.commit();
    Ok(report)
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "the public runtime shares the empty-vehicle phase runner"
)]
pub(crate) struct EmptyVehicles;
impl VehiclePhases for EmptyVehicles {
    fn admit(&self, world: &World) -> Result<(), WorldTickError> {
        if world
            .tables()
            .get(b"VEHS")
            .is_none_or(|table| !table.records().is_empty())
        {
            return Err(unsupported("domain", "nonempty VEHS"));
        }
        Ok(())
    }
    fn economy_boundary(&self, _: &ClockEvents) -> Result<(), WorldTickError> {
        Ok(())
    }
    fn calendar(
        &mut self,
        _: &mut State<'_>,
        _: &ClockState,
        _: bool,
    ) -> Result<(), WorldTickError> {
        Ok(())
    }
    fn ticks(&mut self, _: &mut State<'_>, _: &ClockState) -> Result<(), WorldTickError> {
        Ok(())
    }
}

impl State<'_> {
    pub(crate) fn plan<'w>(
        world: &'w mut World,
        ticks: u32,
        vehicles: &mut impl VehiclePhases,
    ) -> Result<
        (
            PreparedWorldTransaction<'w>,
            WorldTickReport,
            Option<crate::runtime::TownHouseCaches>,
        ),
        WorldTickError,
    > {
        if ticks > MAX_TICKS {
            return Err(unsupported("budget", "tick limit"));
        }
        let paused = unsigned(world, b"DATE", 0, "pause_mode")? != 0;
        let initial_roads = domain::validate(world, paused, vehicles)?;
        let mut clock = restore_clock(world)?;
        // A pending link job may release pause during this horizon. Defer any active-domain
        // error until that actually happens; an unrelated pause still admits only paused work.
        let mut active_domain = Some((|| {
            let roads_admitted = if paused {
                domain::validate(world, false, vehicles)?
            } else {
                initial_roads
            };
            vehicles.admit(world)?;
            Ok::<_, WorldTickError>((
                Landscape::for_world(
                    map(world),
                    u32::try_from(unsigned(world, b"DATE", 0, "cur_tileloop_tile")?)
                        .map_err(|_| unsupported("landscape", "cursor"))?,
                    roads_admitted,
                    vehicles.station_tiles(),
                )?
                .into_scheduler(),
                crate::runtime::TownHouseCaches::restore(world)?,
            ))
        })());
        let mut landscape = None;
        let mut state = State::load(world, None);
        let mut report = WorldTickReport { ticks: Vec::new() };
        for step in 0..ticks {
            vehicles.before_state_loop(&mut state)?;
            let paused = state.number(b"DATE", 0, "pause_mode")? != 0;
            if !paused && landscape.is_none() {
                let (scheduler, towns) = active_domain
                    .take()
                    .ok_or_else(|| unsupported("landscape", "missing active admission"))??;
                landscape = Some(scheduler);
                state.towns = Some(towns);
            }
            let events = advance(&mut state, &mut clock, &mut landscape, vehicles, paused)
                .map_err(|error| match error {
                    WorldTickError::Unsupported { phase, reason } => {
                        unsupported(phase, &format!("call {step}: {reason}"))
                    }
                    other => other,
                })?;
            report.ticks.push(events);
        }
        let (candidate, towns) = state.finish()?;
        Ok((candidate, report, towns))
    }
}

fn advance(
    state: &mut State<'_>,
    clock: &mut ClockState,
    landscape: &mut Option<crate::TileLoop>,
    vehicles: &mut impl VehiclePhases,
    paused: bool,
) -> Result<TickEvents, WorldTickError> {
    let before = clock.snapshot();
    let events = clock.try_advance(paused, |phase, clock, events| {
        save_clock(state, clock.snapshot())?;
        if clock.cache().calendar_year >= 5_000_000 || clock.cache().economy_year >= 5_000_000 {
            return Err(unsupported("clock", "maximum-year runtime cache"));
        }
        match phase {
            ClockPhase::Calendar => vehicles.calendar(state, clock, events.calendar_progressed)?,
            ClockPhase::Economy => {
                vehicles.economy_boundary(events)?;
                for event in events.iter() {
                    match event {
                        ClockEvent::EconomyDay => daily(state)?,
                        ClockEvent::EconomyMonth => {
                            finance::monthly(state, clock.cache().economy_month, vehicles)?;
                            industry_month(
                                state,
                                clock.cache().economy_month,
                                clock.cache().economy_year,
                                before.days_since_last_month.wrapping_add(1),
                            )?;
                            vehicles.station_month(state)?;
                            towns::monthly(state, clock.cache().economy_month)?;
                        }
                        ClockEvent::EconomyYear => {
                            houses::yearly(state)?;
                            finance::yearly(state)?;
                        }
                        ClockEvent::CalendarDay
                        | ClockEvent::CalendarMonth
                        | ClockEvent::CalendarYear
                        | ClockEvent::EconomyWeek
                        | ClockEvent::EconomyQuarter => {}
                    }
                }
            }
            ClockPhase::Tick => competitor(state)?,
        }
        Ok(())
    })?;
    if !paused {
        if let Some(landscape) = landscape {
            landscape.advance(clock.snapshot().tick_counter.0, |index| {
                let index =
                    u32::try_from(index).map_err(|_| unsupported("landscape", "tile index"))?;
                let before = state.view().tile(index)?;
                if before.tile_type() >> 4 == 3 {
                    return houses::visit(state, index, clock.snapshot().tick_counter.0, vehicles);
                }
                let mut tile = tile(&before);
                if tile.tile_type >> 4 == 4 {
                    trees::visit(state, &mut tile, index, clock.snapshot().tick_counter.0)?;
                } else if tile.tile_type >> 4 == 2 {
                    houses::road(state, &mut tile, index)?;
                } else {
                    tile.visit_landscape();
                }
                let after = tiles::saved(&tile);
                if before != after {
                    state.apply(WorldEdit::Tile {
                        index,
                        value: after,
                    })?;
                }
                Ok::<(), WorldTickError>(())
            })?;
            state.set(
                b"DATE",
                0,
                "cur_tileloop_tile",
                WireValue::Unsigned(
                    u64::try_from(landscape.cursor())
                        .map_err(|_| unsupported("landscape", "cursor"))?,
                ),
            )?;
        }
        vehicles.ticks(state, clock)?;
        trees::tick(state, clock.snapshot().tick_counter.0)?;
        vehicles.stations(state)?;
        phases::company_tick(state)?;
        state.set_number(
            b"DATE",
            0,
            "company_tick_counter",
            state
                .number(b"DATE", 0, "company_tick_counter")?
                .wrapping_add(1)
                % 15,
        )?;
        vehicles.link_graphs(state)?;
    }
    finance::limits(state)?;
    Ok(TickEvents {
        tick_counter: clock.snapshot().tick_counter.0,
        calendar_progressed: events.calendar_progressed,
        events: events.iter().map(|event| event.name().into()).collect(),
    })
}
