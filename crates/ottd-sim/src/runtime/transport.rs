//! Native payment/link pool history and non-saved payment/job restoration state.
use super::{
    RuntimeError, SavedVehicleView, VehicleId, pools::PoolAllocator, stations::saved::StationRow,
};
use ottd_save::world::{CandidateView, World};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct EmptyJob {
    pub nodes: usize,
    pub edges: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TransportRuntime {
    pub payments: PoolAllocator,
    pub graphs: PoolAllocator,
    pub jobs: PoolAllocator,
    pub payment_stations: BTreeMap<u32, u16>,
    pub job_results: BTreeMap<u32, Option<EmptyJob>>,
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
        let job_results = jobs
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
        Ok(Self {
            payments: PoolAllocator::restore(0xff000, 512, payments.records().keys().copied())?,
            graphs: PoolAllocator::restore(65535, 32, graphs.records().keys().copied())?,
            jobs: PoolAllocator::restore(65535, 32, jobs.records().keys().copied())?,
            payment_stations,
            job_results,
        })
    }
    pub(super) fn validate(&self, view: CandidateView<'_>) -> Result<(), RuntimeError> {
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
            || self.jobs.snapshot().occupied != self.job_results.keys().copied().collect::<Vec<_>>()
        {
            return Err(RuntimeError::Invalid(
                "tentative transport cache publication",
            ));
        }
        Ok(())
    }
    pub(super) fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"payments":self.payments.snapshot(),"graphs":self.graphs.snapshot(),"jobs":self.jobs.snapshot(),
            "payment_stations":self.payment_stations,"job_results":self.job_results})
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
