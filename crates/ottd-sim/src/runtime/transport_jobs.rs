//! Parsed zero-demand inputs and the bounded native handler pipeline.
use super::{
    RuntimeError,
    job_executor::{Worker, WorkerError},
    stations::saved::StationRow,
    transport::EmptyJob,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct EmptyInput {
    edges_per_node: Vec<usize>,
}
impl EmptyInput {
    pub(super) fn parse(graph: StationRow<'_>) -> Result<Option<Self>, RuntimeError> {
        let (schema, rows) = graph.rows("nodes")?;
        let mut edges_per_node = Vec::with_capacity(rows.len());
        for record in rows {
            let node = StationRow { schema, record };
            if node.number("supply")? != 0 || node.number("demand")? != 0 {
                return Ok(None);
            }
            let (edge_schema, edges) = node.rows("edges")?;
            for record in edges {
                let edge = StationRow {
                    schema: edge_schema,
                    record,
                };
                if edge.number("usage")? != 0 || edge.number("capacity")? == 0 {
                    return Ok(None);
                }
            }
            edges_per_node.push(edges.len());
        }
        Ok(Some(Self { edges_per_node }))
    }
    pub(super) fn spawn(self) -> Arc<Worker<EmptyJob>> {
        Arc::new(Worker::spawn(move |cancel| self.run(cancel)))
    }
    fn run(&self, cancel: &AtomicBool) -> Result<EmptyJob, WorkerError> {
        // Init allocates node/edge annotations; the zero-demand pipeline has no paths or flows.
        let mut edges = 0_usize;
        for count in &self.edges_per_node {
            if cancel.load(Ordering::Acquire) {
                return Err(WorkerError::Cancelled);
            }
            edges = edges.saturating_add(*count);
        }
        // Run checks cancellation before each remaining native handler.
        for _ in 0..5 {
            if cancel.load(Ordering::Acquire) {
                return Err(WorkerError::Cancelled);
            }
        }
        Ok(EmptyJob {
            nodes: self.edges_per_node.len(),
            edges,
        })
    }
}
impl From<WorkerError> for RuntimeError {
    fn from(error: WorkerError) -> Self {
        Self::Invalid(match error {
            WorkerError::Panicked => "link graph worker panicked",
            WorkerError::Synchronization => "link graph worker synchronization",
            WorkerError::Cancelled => "link graph worker cancelled",
        })
    }
}

/// Native `SpawnAll` follows LGRS running order, not pool identifier order.
pub(super) fn restore_workers(
    world: &ottd_save::world::World,
) -> Result<std::collections::BTreeMap<u32, Arc<Worker<EmptyJob>>>, RuntimeError> {
    let ottd_save::WireValue::Array(running) =
        crate::world_access::field(world, b"LGRS", 0, "running")
            .map_err(|_| RuntimeError::Invalid("saved running jobs"))?
    else {
        return Err(RuntimeError::Invalid("saved running job list"));
    };
    let mut inputs = Vec::with_capacity(running.len());
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
        let row = super::stations::saved::row(world, *b"LGRJ", id)?;
        inputs.push((id, EmptyInput::parse(row.child("linkgraph")?)?));
    }
    // Validate every input before starting any host execution.
    let mut workers = std::collections::BTreeMap::new();
    for (id, input) in inputs {
        if let Some(input) = input {
            workers.insert(id, input.spawn());
        }
    }
    Ok(workers)
}
#[cfg(test)]
mod tests;
