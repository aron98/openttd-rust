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
    let mut world = initial.clone();
    let mut cursor = cursor.clone();
    checkpoint(&world, "initial", observer)?;
    while let Some(action) = cursor.next().cloned() {
        if through.is_some_and(|end| action.ordinal() > end) {
            break;
        }
        let before = runtime::observe(&world)?;
        let (op, receipt) = match &action {
            ReplayAction::Command { request, .. } => {
                ("command", Some(execute_command(&mut world, request)?))
            }
            ReplayAction::Tick { count, .. } => {
                advance_world(&mut world, *count)?;
                ("tick", None)
            }
            ReplayAction::Checkpoint { label, .. } => {
                checkpoint(&world, label, observer)?;
                ("checkpoint", None)
            }
        };
        let after = runtime::observe(&world)?;
        observer(
            &ReplayEvent::Action(&ReplayObservation {
                ordinal: action.ordinal(),
                op,
                before,
                after,
                receipt,
            }),
            &world,
        )?;
        cursor.advance()?;
    }
    checkpoint(&world, "final", observer)?;
    Ok(ReplayOutcome { world, cursor })
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
