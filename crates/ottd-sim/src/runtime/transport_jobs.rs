//! Owned immutable link-job inputs and real native handler execution.
mod dijkstra;
mod input;
mod model;
mod paths;
mod pipeline;
use super::{
    RuntimeError,
    job_executor::{Worker, WorkerError},
};
pub(super) use input::{JobEnvironment, graph_supported};
pub(super) use model::{JobInput, JobResult};
use std::sync::Arc;
impl From<WorkerError> for RuntimeError {
    fn from(error: WorkerError) -> Self {
        Self::Invalid(match error {
            WorkerError::InvalidInput => "link graph worker input/annotation invariant",
            WorkerError::Panicked => "link graph worker panicked",
            WorkerError::Synchronization => "link graph worker synchronization",
            WorkerError::Cancelled => "link graph worker cancelled",
        })
    }
}
/// Native `SpawnAll` follows LGRS running order, not pool identifier order.
pub(super) fn restore_workers(
    world: &ottd_save::world::World,
    inputs: &std::collections::BTreeMap<u32, Option<JobInput>>,
) -> Result<std::collections::BTreeMap<u32, Arc<Worker<JobResult>>>, RuntimeError> {
    let ottd_save::WireValue::Array(running) =
        crate::world_access::field(world, b"LGRS", 0, "running")
            .map_err(|_| RuntimeError::Invalid("saved running jobs"))?
    else {
        return Err(RuntimeError::Invalid("saved running job list"));
    };
    let mut ordered = Vec::with_capacity(running.len());
    let mut seen = std::collections::BTreeSet::new();
    for pointer in running {
        let ottd_save::WireValue::Unsigned(pointer) = pointer else {
            return Err(RuntimeError::Invalid("saved job pointer"));
        };
        let id = u32::try_from(
            pointer
                .checked_sub(1)
                .ok_or(RuntimeError::Invalid("null job pointer"))?,
        )
        .map_err(|_| RuntimeError::Invalid("saved job identifier"))?;
        if !seen.insert(id) {
            return Err(RuntimeError::Invalid("duplicate running job"));
        }
        ordered.push((
            id,
            inputs
                .get(&id)
                .ok_or(RuntimeError::Invalid("missing job input"))?
                .clone(),
        ));
    }
    // Validate every input before starting any host execution.
    let mut workers = std::collections::BTreeMap::new();
    for (id, input) in ordered {
        if let Some(input) = input {
            workers.insert(id, input.spawn());
        }
    }
    Ok(workers)
}
#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod path_tests;
#[cfg(test)]
mod tests;
