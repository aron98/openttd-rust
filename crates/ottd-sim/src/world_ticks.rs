//! Transactional native state-loop execution for explicitly admitted saved worlds.
mod access;
mod clocks;
mod company_history;
mod domain;
mod finance;
mod phases;
use clocks::{restore_clock, save_clock};
use phases::{competitor, daily, industry_month};
mod towns;
use crate::{
    Landscape, MAX_TICKS, Map, TickEvents, Tile,
    world_access::{WorldAccessError, unsigned},
};
use access::State;
use ottd_core::{ClockEvent, ClockState};
use ottd_save::{
    TileRawParts, WireValue,
    world::{World, WorldEdit},
};
use serde::{Deserialize, Serialize};

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
fn unsupported(phase: &'static str, reason: &str) -> WorldTickError {
    WorldTickError::Unsupported {
        phase,
        reason: reason.into(),
    }
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
    if ticks > MAX_TICKS {
        return Err(unsupported("budget", "tick limit"));
    }
    if ticks == 0 {
        return Ok(WorldTickReport { ticks: Vec::new() });
    }
    let paused = unsigned(world, b"DATE", 0, "pause_mode")? != 0;
    let roads_admitted = domain::validate(world, paused)?;
    let mut state = State::load(world);
    let mut clock = restore_clock(world)?;
    let mut landscape = if paused {
        None
    } else {
        Some(Landscape::for_world(
            map(world),
            u32::try_from(unsigned(world, b"DATE", 0, "cur_tileloop_tile")?)
                .map_err(|_| unsupported("landscape", "cursor"))?,
            roads_admitted,
        )?)
    };
    let mut report = WorldTickReport { ticks: Vec::new() };
    for step in 0..ticks {
        let result = advance(&mut state, &mut clock, &mut landscape, world, paused);
        let events = result.map_err(|error| match error {
            WorldTickError::Unsupported { phase, reason } => {
                unsupported(phase, &format!("call {step}: {reason}"))
            }
            other => other,
        })?;
        report.ticks.push(events);
    }
    save_clock(&mut state, clock.snapshot())?;
    let mut edits = state.edits(world)?;
    if let Some(landscape) = landscape {
        edits.push(crate::world_access::field_edit(
            *b"DATE",
            0,
            "cur_tileloop_tile",
            WireValue::Unsigned(
                u64::try_from(landscape.cursor())
                    .map_err(|_| unsupported("landscape", "cursor"))?,
            ),
        ));
        for (index, (before, after)) in world
            .map()
            .tiles()
            .iter()
            .zip(landscape.map().tiles.iter())
            .enumerate()
        {
            let next = ottd_save::TileState::from(TileRawParts {
                tile_type: after.tile_type,
                height: after.height,
                m1: after.m1,
                m2: after.m2,
                m3: after.m3,
                m4: after.m4,
                m5: after.m5,
                m6: after.m6,
                m7: after.m7,
                m8: after.m8,
            });
            if *before != next {
                edits.push(WorldEdit::Tile {
                    index: u32::try_from(index)
                        .map_err(|_| unsupported("landscape", "tile index"))?,
                    value: next,
                });
            }
        }
    }
    world.edit_batch(edits)?;
    Ok(report)
}

fn advance(
    state: &mut State,
    clock: &mut ClockState,
    landscape: &mut Option<Landscape>,
    world: &World,
    paused: bool,
) -> Result<TickEvents, WorldTickError> {
    let before = clock.snapshot();
    let events = clock.advance(paused);
    if !paused {
        if clock.cache().calendar_year >= 5_000_000 || clock.cache().economy_year >= 5_000_000 {
            return Err(unsupported("clock", "maximum-year runtime cache"));
        }
        for event in events.iter() {
            match event {
                ClockEvent::EconomyDay => daily(state, world)?,
                ClockEvent::EconomyMonth => {
                    finance::monthly(state, world, clock.cache().economy_month)?;
                    industry_month(
                        state,
                        world,
                        clock.cache().economy_month,
                        clock.cache().economy_year,
                        before.days_since_last_month.wrapping_add(1),
                    )?;
                    towns::monthly(state, clock.cache().economy_month)?;
                }
                ClockEvent::EconomyYear => finance::yearly(state)?,
                ClockEvent::CalendarDay
                | ClockEvent::CalendarMonth
                | ClockEvent::CalendarYear
                | ClockEvent::EconomyWeek
                | ClockEvent::EconomyQuarter => {}
            }
        }
        competitor(state)?;
        if let Some(landscape) = landscape {
            landscape.advance(clock.snapshot().tick_counter.0);
        }
        state.set_number(
            b"DATE",
            0,
            "company_tick_counter",
            state
                .number(b"DATE", 0, "company_tick_counter")?
                .wrapping_add(1)
                % 15,
        )?;
    }
    finance::limits(state, world)?;
    Ok(TickEvents {
        tick_counter: clock.snapshot().tick_counter.0,
        calendar_progressed: events.calendar_progressed,
        events: events.iter().map(|event| event.name().into()).collect(),
    })
}

fn map(world: &World) -> Map {
    Map {
        width: world.map().width(),
        height: world.map().height(),
        tiles: world
            .map()
            .tiles()
            .iter()
            .map(|t| Tile {
                tile_type: t.tile_type(),
                height: t.height(),
                m1: t.m1(),
                m2: t.m2(),
                m3: t.m3(),
                m4: t.m4(),
                m5: t.m5(),
                m6: t.m6(),
                m7: t.m7(),
                m8: t.m8(),
            })
            .collect(),
    }
}
