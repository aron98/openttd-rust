//! Native payment/link pool history and non-saved payment/job restoration state.
use super::job_executor::Worker;
#[path = "cargo.rs"]
pub(super) mod cargo;
use super::{
    RuntimeError, SavedVehicleView, VehicleId, pools::PoolAllocator, stations::saved::StationRow,
};
use cargo::CargoRuntime;
use ottd_save::world::{CandidateView, World};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct EmptyJob {
    pub nodes: usize,
    pub edges: usize,
}
#[derive(Debug, Clone)]
pub(super) struct TransportRuntime {
    pub cargo: CargoRuntime,
    pub payments: PoolAllocator,
    pub graphs: PoolAllocator,
    pub jobs: PoolAllocator,
    pub payment_stations: BTreeMap<u32, u16>,
    /// Immutable validation summaries, not live solver completion.
    pub job_expectations: BTreeMap<u32, Option<EmptyJob>>,
    pub workers: BTreeMap<u32, Arc<Worker<EmptyJob>>>,
}
impl Drop for TransportRuntime {
    fn drop(&mut self) {
        // Native Clear aborts all jobs before pool destruction joins them. Candidate leases
        // must leave committed workers alone; only uniquely owned jobs belong to this cleanup.
        for worker in self.workers.values_mut() {
            if let Some(worker) = Arc::get_mut(worker) {
                worker.cancel_unfinished();
            }
        }
    }
}
impl TransportRuntime {
    pub(super) fn restore(world: &World) -> Result<Self, RuntimeError> {
        let table = |key| {
            world
                .tables()
                .get(&key)
                .ok_or(RuntimeError::Invalid("transport pool"))
        };
        let payments = table(*b"CAPY")?;
        let graphs = table(*b"LGRP")?;
        let jobs = table(*b"LGRJ")?;
        let job_expectations = jobs
            .records()
            .iter()
            .map(|(id, record)| {
                Ok((
                    *id,
                    empty_job(
                        StationRow {
                            schema: jobs.schema(),
                            record,
                        }
                        .child("linkgraph")?,
                    )?,
                ))
            })
            .collect::<Result<_, RuntimeError>>()?;
        let payment_stations = payments
            .records()
            .iter()
            .map(|(id, record)| {
                let front = (StationRow {
                    schema: payments.schema(),
                    record,
                })
                .number("front")?;
                let vehicle = u32::try_from(
                    front
                        .checked_sub(1)
                        .ok_or(RuntimeError::Invalid("payment front"))?,
                )
                .map_err(|_| RuntimeError::Invalid("payment vehicle ID"))?;
                let station = SavedVehicleView::new(world, VehicleId::new(vehicle))?
                    .common_number("last_station_visited")?;
                Ok((
                    *id,
                    u16::try_from(station).map_err(|_| RuntimeError::Invalid("payment station"))?,
                ))
            })
            .collect::<Result<_, RuntimeError>>()?;
        let payments = PoolAllocator::restore(0xff000, 512, payments.records().keys().copied())?;
        let graphs = PoolAllocator::restore(65535, 32, graphs.records().keys().copied())?;
        let jobs = PoolAllocator::restore(65535, 32, jobs.records().keys().copied())?;
        let cargo = CargoRuntime::restore(world)?;
        let workers = super::transport_jobs::restore_workers(world)?;
        Ok(Self {
            cargo,
            payments,
            graphs,
            jobs,
            payment_stations,
            job_expectations,
            workers,
        })
    }
    pub(super) fn validate(&self, view: CandidateView<'_>) -> Result<(), RuntimeError> {
        self.cargo.validate(view)?;
        for (chunk, pool) in [
            (*b"CAPY", &self.payments),
            (*b"LGRP", &self.graphs),
            (*b"LGRJ", &self.jobs),
        ] {
            let ids: Vec<_> = view
                .table(chunk)
                .ok_or(RuntimeError::Invalid("transport pool"))?
                .records()
                .map(|(id, _)| id)
                .collect();
            if pool.snapshot().occupied != ids {
                return Err(RuntimeError::Invalid(
                    "tentative transport pool publication",
                ));
            }
        }
        if self.payments.snapshot().occupied
            != self.payment_stations.keys().copied().collect::<Vec<_>>()
            || self.jobs.snapshot().occupied
                != self.job_expectations.keys().copied().collect::<Vec<_>>()
        {
            return Err(RuntimeError::Invalid(
                "tentative transport cache publication",
            ));
        }
        let executable: Vec<_> = self
            .job_expectations
            .iter()
            .filter_map(|(id, result)| result.as_ref().map(|_| *id))
            .collect();
        if executable != self.workers.keys().copied().collect::<Vec<_>>() {
            return Err(RuntimeError::Invalid("tentative worker ownership"));
        }
        Ok(())
    }
    pub(super) fn snapshot(&self) -> serde_json::Value {
        let results: BTreeMap<_, _> = self
            .workers
            .iter()
            .map(|(id, worker)| {
                let value = match worker.poll() {
                    Ok(Some(value)) => serde_json::json!(*value),
                    Ok(None) => serde_json::Value::Null,
                    Err(error) => serde_json::json!({"worker_error":error.to_string()}),
                };
                (*id, value)
            })
            .collect();
        let mut snapshot = serde_json::Map::from_iter([
            ("cargo".into(), self.cargo.snapshot()),
            (
                "payments".into(),
                serde_json::json!(self.payments.snapshot()),
            ),
            ("graphs".into(), serde_json::json!(self.graphs.snapshot())),
            ("jobs".into(), serde_json::json!(self.jobs.snapshot())),
            (
                "payment_stations".into(),
                serde_json::json!(self.payment_stations),
            ),
            ("job_results".into(), serde_json::json!(results)),
        ]);
        let unsupported: Vec<_> = self
            .job_expectations
            .keys()
            .filter_map(|id| (!self.workers.contains_key(id)).then_some(*id))
            .collect();
        if !unsupported.is_empty() {
            snapshot.insert("unsupported_jobs".into(), serde_json::json!(unsupported));
        }
        serde_json::Value::Object(snapshot)
    }
}
pub(super) fn empty_job(graph: StationRow<'_>) -> Result<Option<EmptyJob>, RuntimeError> {
    let (schema, rows) = graph.rows("nodes")?;
    let mut edges = 0_usize;
    for record in rows {
        let node = StationRow { schema, record };
        if node.number("supply")? != 0 || node.number("demand")? != 0 {
            return Ok(None);
        }
        let (edge_schema, edge_rows) = node.rows("edges")?;
        for record in edge_rows {
            let edge = StationRow {
                schema: edge_schema,
                record,
            };
            if edge.number("usage")? != 0 || edge.number("capacity")? == 0 {
                return Ok(None);
            }
        }
        edges = edges.saturating_add(node.rows("edges")?.1.len());
    }
    // Init creates annotations; Demand has no supply/demand and both MCF passes have no paths.
    // Both FlowMapper passes consequently produce empty per-node flow maps.
    Ok(Some(EmptyJob {
        nodes: rows.len(),
        edges,
    }))
}

impl super::SimulationRuntime {
    /// Live transport pool history and payment/job observations.
    /// Worker completion is timing-dependent and may change without advancing the World.
    /// Unsupported populated jobs are identified explicitly, never reported as pending workers.
    pub fn transport_state_json(&self) -> serde_json::Value {
        self.transport.snapshot()
    }
}
