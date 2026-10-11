#![expect(
    clippy::redundant_pub_crate,
    reason = "station transaction adapters are shared internally with commands"
)]
use super::{RuntimeError, StationRuntime};
use crate::content::ContentCatalog;
use ottd_save::world::{World, WorldEdit};
use std::collections::BTreeMap;

pub(crate) struct StationPublication {
    pub next: StationRuntime,
    pub company: u8,
    pub station: u32,
}
pub(crate) struct StationContext<'a> {
    pub content: &'a ContentCatalog,
    pub stations: &'a mut StationRuntime,
    pub road: &'a mut BTreeMap<u8, [u32; 63]>,
}
impl StationContext<'_> {
    pub(crate) fn publish(
        self,
        world: &mut World,
        edits: Vec<WorldEdit>,
        publication: StationPublication,
    ) -> Result<(), RuntimeError> {
        let StationPublication {
            next,
            company,
            station,
        } = publication;
        let dimensions = (world.map().width(), world.map().height());
        let radius = super::catchment_radius(world)?;
        let road = self
            .road
            .get_mut(&company)
            .and_then(|r| r.first_mut())
            .ok_or(RuntimeError::Invalid("station road infrastructure"))?;
        let pieces = road.checked_add(2).ok_or(RuntimeError::Invalid(
            "station road infrastructure overflow",
        ))?;
        let mut transaction = world.transaction();
        for edit in edits {
            transaction.apply(edit)?;
        }
        let prepared = transaction.prepare()?;
        for (chunk, pool) in [(*b"STNN", &next.stations), (*b"ROAD", &next.stops)] {
            let occupied = prepared
                .view()
                .table(chunk)
                .ok_or(RuntimeError::Invalid("station candidate table"))?
                .records()
                .map(|(id, _)| id)
                .collect::<Vec<_>>();
            if occupied != pool.snapshot().occupied {
                return Err(RuntimeError::Invalid("station allocation membership"));
            }
        }
        let geometry = super::restore_geometry(
            super::saved::StationRead::Candidate(prepared.view()),
            station,
            dimensions,
            radius,
        )?;
        if next.geometry.get(&station) != Some(&geometry) {
            return Err(RuntimeError::Invalid("station candidate geometry"));
        }
        prepared.commit();
        *self.stations = next;
        *road = pieces;
        Ok(())
    }
}
