//! Native valid-node goods publication and final empty-data destruction.
use super::{State, WorldTickError, records, unsupported};
use crate::runtime::{
    transport::cargo::CargoRuntime,
    transport_jobs::{JobInput, JobResult},
};

pub(super) struct Publication<'a, 'w> {
    pub state: &'a mut State<'w>,
    pub cargo: &'a mut CargoRuntime,
}
impl JobResult {
    pub(super) fn publish(
        &self,
        input: &JobInput,
        target: &mut Publication<'_, '_>,
    ) -> Result<(), WorldTickError> {
        self.validate(input)
            .map_err(crate::runtime::RuntimeError::from)?;
        if target
            .state
            .table(b"LGRP")?
            .record(u32::from(input.graph))
            .is_none()
        {
            return Ok(());
        }
        for (index, node) in input.nodes.iter().enumerate() {
            let station = u32::from(node.station);
            if target.state.table(b"STNN")?.record(station).is_none() {
                continue;
            }
            if records::row(target.state, *b"STNN", station)?.number("facilities")? & 128 != 0 {
                continue;
            }
            let goods = records::goods(target.state, station, input.cargo)?;
            if goods.number("link_graph")? != u64::from(input.graph)
                || goods.number("node")?
                    != u64::try_from(index).map_err(|_| unsupported("link_jobs", "node width"))?
            {
                continue;
            }
            // New manual annotations have no flows. Existing flows require the complete
            // invalidate/swap/reroute consumer, never a silent empty-map substitution.
            if !goods.rows("flow")?.1.is_empty() {
                return Err(unsupported(
                    "link_jobs",
                    "existing flow publication/rerouting",
                ));
            }
            let entries = target
                .cargo
                .stations
                .get_mut(&station)
                .ok_or_else(|| unsupported("link_jobs", "station cargo cache"))?;
            let cargo = u32::from(input.cargo);
            let entry = entries.entry(cargo).or_default();
            // GoodsEntryData::IsEmpty uses TotalCount, including reserved cargo.
            if entry.count.wrapping_add(entry.reserved) == 0 {
                entries.remove(&cargo);
            }
        }
        Ok(())
    }
}
