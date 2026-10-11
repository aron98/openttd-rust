//! Candidate planning and lazy active-domain restoration after job pause release.
use super::{State, VehiclePhases, WorldTickError, WorldTickReport, advance, domain, unsupported};
use super::{clocks::restore_clock, tiles::map};
use crate::world_access::unsigned;
use crate::{Landscape, MAX_TICKS};
use ottd_save::world::{PreparedWorldTransaction, World};

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
        let industries = crate::runtime::IndustryCaches::restore(world)?;
        let mut state = State::load(world, None, industries);
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
