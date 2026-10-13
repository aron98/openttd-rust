//! Execute the six native handlers for manual and existing empty automatic jobs.
use super::{
    dijkstra::Search,
    model::{Annotation, Distribution, EdgeFlow, JobInput, JobResult},
    paths::{self, Metric},
};
use crate::runtime::job_executor::{Worker, WorkerError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
type Result<T> = std::result::Result<T, WorkerError>;

impl JobInput {
    pub(in crate::runtime) fn spawn(self) -> Arc<Worker<JobResult>> {
        Arc::new(Worker::spawn(move |cancel| self.run(cancel)))
    }
    pub(in crate::runtime) fn run(&self, cancel: &AtomicBool) -> Result<JobResult> {
        check_abort(cancel)?;
        let mut job = JobResult {
            nodes: self.nodes.len(),
            edges: 0,
            annotations: Vec::with_capacity(self.nodes.len()),
        };
        for node in &self.nodes {
            job.edges = job.edges.wrapping_add(node.edges.len());
            job.annotations.push(Annotation {
                undelivered_supply: node.supply,
                demands: vec![[0, 0]; self.nodes.len()],
                edge_flows: node
                    .edges
                    .iter()
                    .map(|e| EdgeFlow {
                        destination: e.destination,
                        flow: 0,
                    })
                    .collect(),
                paths: Vec::new(),
                flows: Vec::new(),
            });
        }
        check_abort(cancel)?;
        demand(self)?;
        check_abort(cancel)?;
        solve(self, &job, Metric::Distance)?;
        // EliminateCycles visits node path lists; no assigned demand registered a path.
        for node in &job.annotations {
            if !node.paths.is_empty() {
                return Err(WorkerError::InvalidInput);
            }
        }
        check_abort(cancel)?;
        map_flows(self, &mut job, false)?;
        check_abort(cancel)?;
        check_abort(cancel)?; // The second MCF pass checks abort on entry to its demand loop.
        solve(self, &job, Metric::Capacity)?;
        check_abort(cancel)?;
        map_flows(self, &mut job, true)?;
        job.validate(self)?;
        Ok(job)
    }
}
fn check_abort(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(WorkerError::Cancelled)
    } else {
        Ok(())
    }
}
fn demand(input: &JobInput) -> Result<()> {
    let dx = input.width.wrapping_sub(1);
    let dy = input.height.wrapping_sub(1);
    let square = dx.max(dy).wrapping_mul(2).wrapping_add(dx.min(dy));
    let root = square.isqrt();
    // IntSqrt rounds to nearest, rather than using the integer floor alone.
    let _base_distance = root.wrapping_add(u32::from(
        square.wrapping_sub(root.wrapping_mul(root)) > root,
    ));
    let modifier = i32::from(input.settings.demand_distance);
    let _modifier = if modifier > 100 {
        100_i32.wrapping_add(modifier.wrapping_sub(100).wrapping_pow(2) / 12)
    } else {
        modifier
    };
    match input.settings.distribution {
        Distribution::Manual => Ok(()),
        Distribution::Asymmetric | Distribution::Symmetric => {
            // CalcDemand scans source/destination queues before its zero-count return.
            let supplies = input.nodes.iter().filter(|node| node.supply > 0).count();
            let demands = input.nodes.iter().filter(|node| node.demand > 0).count();
            if supplies == 0 || demands == 0 {
                Ok(())
            } else {
                Err(WorkerError::InvalidInput)
            }
        }
    }
}
fn solve(input: &JobInput, result: &JobResult, metric: Metric) -> Result<()> {
    let mut finished = vec![false; input.nodes.len()];
    for (source, done) in finished.iter_mut().enumerate() {
        if *done {
            continue;
        }
        let source = u16::try_from(source).map_err(|_| WorkerError::InvalidInput)?;
        let mut paths = Search {
            input,
            result,
            metric,
        }
        .run(source)?;
        let node = result
            .annotations
            .get(usize::from(source))
            .ok_or(WorkerError::InvalidInput)?;
        // A positive unsatisfied demand needs the automatic PushFlow/cycle consumer.
        if node.demands.iter().any(|demand| demand[1] > 0) {
            return Err(WorkerError::InvalidInput);
        }
        *done = true;
        paths::cleanup(&mut paths, source)?;
    }
    Ok(())
}
fn map_flows(input: &JobInput, result: &mut JobResult, scale: bool) -> Result<()> {
    for node in &result.annotations {
        if node.paths.first().is_some_and(|path| path.flow != 0) {
            return Err(WorkerError::InvalidInput);
        }
    }
    for node in &mut result.annotations {
        // FinalizeLocalConsumption traverses the actual empty flow map.
        if !node.flows.is_empty() {
            return Err(WorkerError::InvalidInput);
        }
        if scale {
            let _runtime = input
                .join_date
                .wrapping_sub(i32::from(input.settings.recalc_time) / 2)
                .wrapping_sub(input.last_compression)
                .wrapping_add(1);
        }
        node.paths.clear();
    }
    Ok(())
}
impl JobResult {
    pub(in crate::runtime) fn validate(&self, input: &JobInput) -> Result<()> {
        if self.nodes != input.nodes.len()
            || self.annotations.len() != self.nodes
            || self.edges
                != input
                    .nodes
                    .iter()
                    .map(|node| node.edges.len())
                    .sum::<usize>()
        {
            return Err(WorkerError::InvalidInput);
        }
        for (actual, source) in self.annotations.iter().zip(&input.nodes) {
            if actual.undelivered_supply != source.supply
                || actual.demands.len() != self.nodes
                || actual.demands.iter().any(|value| *value != [0, 0])
                || actual.edge_flows.len() != source.edges.len()
                || !actual.paths.is_empty()
                || !actual.flows.is_empty()
                || actual
                    .edge_flows
                    .iter()
                    .zip(&source.edges)
                    .any(|(a, b)| a.destination != b.destination || a.flow != 0)
            {
                return Err(WorkerError::InvalidInput);
            }
        }
        Ok(())
    }
}
