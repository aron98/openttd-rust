//! Source-ordered shortest/most-capacious path construction for each job source.
use super::{
    model::{JobInput, JobResult, Path},
    paths::{self, Leg, Metric},
};
use crate::runtime::job_executor::WorkerError;
use std::collections::BTreeSet;

pub(super) struct Search<'a> {
    pub input: &'a JobInput,
    pub result: &'a JobResult,
    pub metric: Metric,
}
impl Search<'_> {
    pub(super) fn run(&self, source: u16) -> Result<Vec<Option<Path>>, WorkerError> {
        let mut paths = Vec::with_capacity(self.input.nodes.len());
        let mut queue = BTreeSet::new();
        for index in 0..self.input.nodes.len() {
            let id = u16::try_from(index).map_err(|_| WorkerError::InvalidInput)?;
            let path = Path::new(id, id == source);
            queue.insert((path.rank(self.metric), id));
            paths.push(Some(path));
        }
        while let Some((_, from)) = queue.pop_first() {
            for to in self.destinations(source, from)? {
                if to == from {
                    continue;
                }
                let leg = self.leg(from, to)?;
                let base = paths::get(&paths, from)?;
                let dest = paths::get(&paths, to)?;
                if dest.better(base, &leg, self.metric) {
                    queue.remove(&(dest.rank(self.metric), to));
                    paths::fork(&mut paths, (from, to), &leg)?;
                    queue.insert((paths::get(&paths, to)?.rank(self.metric), to));
                }
            }
        }
        Ok(paths)
    }
    fn destinations(&self, source: u16, from: u16) -> Result<Vec<u16>, WorkerError> {
        match self.metric {
            Metric::Distance => Ok(self
                .input
                .nodes
                .get(usize::from(from))
                .ok_or(WorkerError::InvalidInput)?
                .edges
                .iter()
                .map(|e| e.destination)
                .collect()),
            Metric::Capacity => {
                let station = self
                    .input
                    .nodes
                    .get(usize::from(source))
                    .ok_or(WorkerError::InvalidInput)?
                    .station;
                let node = self
                    .result
                    .annotations
                    .get(usize::from(from))
                    .ok_or(WorkerError::InvalidInput)?;
                node.flows
                    .iter()
                    .find(|flow| flow.origin == station)
                    .map_or_else(
                        || Ok(Vec::new()),
                        |flow| {
                            flow.shares
                                .iter()
                                .map(|(_, via)| {
                                    self.input
                                        .nodes
                                        .iter()
                                        .position(|n| n.station == *via)
                                        .and_then(|i| u16::try_from(i).ok())
                                        .ok_or(WorkerError::InvalidInput)
                                })
                                .collect()
                        },
                    )
            }
        }
    }
    fn leg(&self, from: u16, to: u16) -> Result<Leg, WorkerError> {
        let node = self
            .input
            .nodes
            .get(usize::from(from))
            .ok_or(WorkerError::InvalidInput)?;
        let dest = self
            .input
            .nodes
            .get(usize::from(to))
            .ok_or(WorkerError::InvalidInput)?;
        let edge = node
            .edges
            .iter()
            .find(|e| e.destination == to)
            .ok_or(WorkerError::InvalidInput)?;
        let flow = self
            .result
            .annotations
            .get(usize::from(from))
            .ok_or(WorkerError::InvalidInput)?
            .edge_flows
            .iter()
            .find(|e| e.destination == to)
            .ok_or(WorkerError::InvalidInput)?
            .flow;
        let capacity = match self.metric {
            Metric::Distance => (edge
                .capacity
                .wrapping_mul(u32::from(self.input.settings.saturation))
                / 100)
                .max(1),
            Metric::Capacity => edge.capacity,
        };
        let width = std::num::NonZeroU32::new(self.input.width).ok_or(WorkerError::InvalidInput)?;
        let dx = (node.xy % width).abs_diff(dest.xy % width);
        let dy = (node.xy / width).abs_diff(dest.xy / width);
        let distance = dx
            .max(dy)
            .wrapping_mul(2)
            .wrapping_add(dx.min(dy))
            .wrapping_add(1);
        let distance = if self.input.express {
            if edge.travel_time == 0 {
                distance.wrapping_mul(74)
            } else {
                edge.travel_time.wrapping_add(74)
            }
        } else {
            distance
        };
        Ok(Leg {
            capacity,
            free: i32::from_ne_bytes(capacity.wrapping_sub(flow).to_ne_bytes()),
            distance,
        })
    }
}
