//! Owned cargo-list caches and packet-pool history, independent of host workers.
#[cfg(test)]
#[path = "cargo/command_tests.rs"]
mod command_tests;
#[path = "cargo/read.rs"]
pub(in crate::runtime) mod read;
#[path = "cargo/recipients.rs"]
mod recipients;
#[path = "cargo/restore.rs"]
mod restore;
use crate::runtime::{RuntimeError, pools::PoolAllocator};
use ottd_save::world::{CandidateView, World};
use serde::Serialize;
use std::collections::BTreeMap;

type Nearby = BTreeMap<u32, Vec<u32>>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(in crate::runtime) struct VehicleList {
    pub packets: Vec<u32>,
    pub count: u32,
    pub periods: u64,
    pub feeder_share: i64,
    pub actions: [u32; 4],
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(in crate::runtime) struct StationList {
    pub packets: BTreeMap<u32, Vec<u32>>,
    pub count: u32,
    pub periods: u64,
    pub reserved: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime) struct CargoRuntime {
    pub pool: PoolAllocator,
    pub vehicles: BTreeMap<u32, VehicleList>,
    pub stations: BTreeMap<u32, BTreeMap<u32, StationList>>,
    // None records unavailable wider station catchment context; restoration remains usable.
    pub town_near: Option<Nearby>,
    pub industry_near: Option<Nearby>,
}

impl CargoRuntime {
    pub(in crate::runtime) fn restore(world: &World) -> Result<Self, RuntimeError> {
        restore::restore(world)
    }

    pub(in crate::runtime) fn validate(&self, view: CandidateView<'_>) -> Result<(), RuntimeError> {
        let ids = view
            .table(*b"CAPA")
            .ok_or(RuntimeError::Invalid("cargo packet pool"))?
            .records()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if ids != self.pool.snapshot().occupied {
            return Err(RuntimeError::Invalid("candidate cargo pool membership"));
        }
        Ok(())
    }

    pub(in crate::runtime) fn station_change(
        &self,
        view: CandidateView<'_>,
        stations: &crate::runtime::stations::StationRuntime,
    ) -> Result<Self, RuntimeError> {
        let mut candidate = self.clone();
        (candidate.town_near, candidate.industry_near) =
            recipients::from_view(read::Read::Candidate(view), &stations.geometry)?;
        for id in read::Read::Candidate(view).ids(*b"STNN")? {
            if !read::Read::Candidate(view)
                .row(*b"STNN", id)?
                .rows("normal")?
                .1
                .is_empty()
            {
                candidate.stations.entry(id).or_default();
            }
        }
        candidate.validate(view)?;
        Ok(candidate)
    }

    pub(in crate::runtime) fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "pool": self.pool.snapshot(), "vehicles": self.vehicles,
            "stations": self.stations, "town_near": self.town_near,
            "industry_near": self.industry_near,
        })
    }
}
