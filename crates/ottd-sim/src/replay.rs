//! Ordered deterministic replay, with candidate-world transactions and host observers.
mod domain;
mod runtime;
mod types;
use crate::{CommandError, CommandReceipt, WorldTickError, advance_world, execute_command};
use ottd_save::world::World;
pub use runtime::ReplayRuntime;
use serde::Serialize;
pub use types::{ReplayAction, ReplayCursor, ReplayPlan};

/// Invalid replay input or an unsupported native operation.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    /// Invalid scheduling, bounds, or checkpoint shape.
    #[error("invalid replay: {0}")]
    Invalid(&'static str),
    /// Loaded state requires native behavior outside the admitted profile.
    #[error("unsupported replay load: {0}")]
    Unsupported(&'static str),
    /// Saved field access failed.
    #[error(transparent)]
    Access(#[from] crate::world_access::WorldAccessError),
    /// Command execution rejected its gameplay context.
    #[error(transparent)]
    Command(#[from] CommandError),
    /// World-loop execution rejected its gameplay context.
    #[error(transparent)]
    Tick(#[from] WorldTickError),
    /// Native runtime restoration rejected the loaded world.
    #[error(transparent)]
    Runtime(#[from] crate::runtime::RuntimeError),
    /// The caller could not retain an observation.
    #[error("replay observer failed: {0}")]
    Observer(String),
}
/// Native-compatible deterministic observation of an action.
#[derive(Debug, Clone, Serialize)]
pub struct ReplayObservation {
    /// Scheduling position, independent of saved simulation ticks.
    pub ordinal: u64,
    /// Native protocol operation tag.
    pub op: &'static str,
    /// Runtime immediately before the action.
    pub before: ReplayRuntime,
    /// Runtime immediately after the action.
    pub after: ReplayRuntime,
    /// Command phases, absent for non-command actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<CommandReceipt>,
}
/// Deterministic host observation; the library performs no filesystem operations.
#[derive(Debug)]
pub enum ReplayEvent<'a> {
    /// One complete native-protocol action observation.
    Action(&'a ReplayObservation),
    /// Save the world at this exact named boundary.
    Checkpoint {
        /// Safe unique basename, including automatic initial/final boundaries.
        label: &'a str,
        /// Runtime cache and saved RNG at that boundary.
        runtime: ReplayRuntime,
    },
}
/// Successful candidate world and its remaining scheduling cursor.
#[derive(Debug)]
pub struct ReplayOutcome {
    /// Final world; the input world is never mutated.
    pub world: World,
    /// Complete plan and position needed to resume under the same budgets.
    pub cursor: ReplayCursor,
}
/// Execute an inclusive ordinal prefix, emitting observations to a host-owned sink.
///
/// # Errors
/// Rejects malformed scheduling, unsupported loaded state, failed observers and
/// unsupported command/tick execution. Input world and cursor remain unchanged.
/// Observers must stage their output privately until this function succeeds.
pub fn run_replay(
    initial: &World,
    cursor: &ReplayCursor,
    through: Option<u64>,
    observer: &mut impl FnMut(&ReplayEvent<'_>, &World) -> Result<(), ReplayError>,
) -> Result<ReplayOutcome, ReplayError> {
    cursor.validate()?;
    if cursor.position() > 0 && through.is_some_and(|end| end < cursor.next_ordinal()) {
        return Err(ReplayError::Invalid("through ordinal is in the past"));
    }
    domain::validate(initial)?;
    let mut owner = ReplayOwner::Saved(Box::new(initial.clone()));
    let mut cursor = cursor.clone();
    checkpoint(owner.world(), "initial", observer)?;
    while let Some(action) = cursor.next().cloned() {
        if through.is_some_and(|end| action.ordinal() > end) {
            break;
        }
        let before = runtime::observe(owner.world())?;
        let (op, receipt) = match &action {
            ReplayAction::Command { request, .. } => ("command", Some(owner.command(request)?)),
            ReplayAction::Tick { count, .. } => {
                owner = owner.advance(*count)?;
                ("tick", None)
            }
            ReplayAction::Checkpoint { label, .. } => {
                checkpoint(owner.world(), label, observer)?;
                ("checkpoint", None)
            }
        };
        let after = runtime::observe(owner.world())?;
        observer(
            &ReplayEvent::Action(&ReplayObservation {
                ordinal: action.ordinal(),
                op,
                before,
                after,
                receipt,
            }),
            owner.world(),
        )?;
        cursor.advance()?;
    }
    checkpoint(owner.world(), "final", observer)?;
    Ok(ReplayOutcome {
        world: owner.into_world(),
        cursor,
    })
}
fn checkpoint(
    world: &World,
    label: &str,
    observer: &mut impl FnMut(&ReplayEvent<'_>, &World) -> Result<(), ReplayError>,
) -> Result<(), ReplayError> {
    observer(
        &ReplayEvent::Checkpoint {
            label,
            runtime: runtime::observe(world)?,
        },
        world,
    )
}

enum ReplayOwner {
    Saved(Box<World>),
    Road(Box<crate::runtime::SimulationRuntime>),
}
impl ReplayOwner {
    fn world(&self) -> &World {
        match self {
            Self::Saved(world) => world,
            Self::Road(runtime) => runtime.world(),
        }
    }
    fn into_world(self) -> World {
        match self {
            Self::Saved(world) => *world,
            Self::Road(runtime) => runtime.into_world(),
        }
    }
    fn command(&mut self, request: &crate::CommandRequest) -> Result<CommandReceipt, CommandError> {
        match self {
            Self::Saved(world) => execute_command(world, request),
            Self::Road(runtime) => runtime.execute_command(request),
        }
    }
    fn advance(self, count: u32) -> Result<Self, ReplayError> {
        match self {
            Self::Saved(mut world) => {
                let needs_road = count > 0
                    && crate::world_access::unsigned(&world, b"DATE", 0, "pause_mode")? == 0
                    && world
                        .tables()
                        .get(b"VEHS")
                        .is_some_and(|table| !table.records().is_empty());
                if needs_road {
                    let mut runtime = crate::runtime::SimulationRuntime::restore_vanilla(*world)?;
                    runtime.advance_world(count)?;
                    Ok(Self::Road(Box::new(runtime)))
                } else {
                    advance_world(&mut world, count)?;
                    Ok(Self::Saved(world))
                }
            }
            Self::Road(mut runtime) => {
                runtime.advance_world(count)?;
                Ok(Self::Road(runtime))
            }
        }
    }
}
