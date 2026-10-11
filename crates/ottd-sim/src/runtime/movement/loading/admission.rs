//! Empty-transport capability checks before any world or runtime publication.
use super::{RoadPhases, SavedVehicleView, WorldTickError, unsupported};
use crate::{
    runtime::{stations::saved, transport_jobs::graph_supported},
    world_access::field,
};
use ottd_save::{WireValue, world::World};
use std::collections::BTreeSet;

pub(in crate::runtime::movement) fn admit(
    world: &World,
    runner: &RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    for name in [
        "linkgraph.distribution_pax",
        "linkgraph.distribution_mail",
        "linkgraph.distribution_armoured",
        "linkgraph.distribution_default",
    ] {
        if crate::world_access::unsigned(world, b"PATS", 0, name)? != 0 {
            return Err(unsupported("transport", "automatic cargo distribution"));
        }
    }
    let vehicle = SavedVehicleView::new(world, runner.id)?;
    let loading = vehicle.common_number("current_order.type")? & 15 == 3;
    let visited = vehicle.common_number("last_station_visited")?;
    let mut queued = 0_usize;
    for id in world
        .tables()
        .get(b"STNN")
        .ok_or_else(|| unsupported("transport", "station pool"))?
        .records()
        .keys()
    {
        let normal = saved::row(world, *b"STNN", *id)?.child("normal")?;
        let WireValue::Array(queue) =
            crate::world_access::row_field(normal.schema, normal.record, "loading_vehicles")?
        else {
            return Err(unsupported("transport", "loading queue"));
        };
        if !queue.is_empty() {
            if !loading
                || visited != u64::from(*id)
                || queue.as_slice()
                    != [WireValue::Unsigned(
                        u64::from(runner.id.raw()).wrapping_add(1),
                    )]
            {
                return Err(unsupported("transport", "loading queue association"));
            }
            queued = queued.wrapping_add(1);
        }
    }
    if queued != usize::from(loading) {
        return Err(unsupported("transport", "loading vehicle queue membership"));
    }
    for (payment, station) in &runner.transport.payment_stations {
        if !loading
            || u64::from(*station) != visited
            || crate::world_access::unsigned(world, b"CAPY", *payment, "front")?
                != u64::from(runner.id.raw()).wrapping_add(1)
        {
            return Err(unsupported(
                "transport",
                "payment front/station association",
            ));
        }
        for name in ["route_profit", "visual_profit", "visual_transfer"] {
            if crate::world_access::signed(world, b"CAPY", *payment, name)? != 0 {
                return Err(unsupported("transport", "nonzero cargo payment"));
            }
        }
    }
    if runner.transport.payment_stations.len() > 1 {
        return Err(unsupported("transport", "duplicate payment front"));
    }
    for id in world
        .tables()
        .get(b"LGRP")
        .ok_or_else(|| unsupported("transport", "graph pool"))?
        .records()
        .keys()
    {
        if !graph_supported(saved::row(world, *b"LGRP", *id)?)? {
            return Err(unsupported("transport", "populated link graph"));
        }
    }
    if runner.transport.job_inputs.values().any(Option::is_none) {
        return Err(unsupported("transport", "populated link graph job"));
    }
    schedule(world, runner)
}
fn schedule(world: &World, runner: &RoadPhases<'_>) -> Result<(), WorldTickError> {
    let mut running = BTreeSet::new();
    for (name, seen) in [
        ("running", &mut running),
        ("schedule", &mut BTreeSet::new()),
    ] {
        let WireValue::Array(values) = field(world, b"LGRS", 0, name)? else {
            return Err(unsupported("transport", "schedule array"));
        };
        for value in values {
            let WireValue::Unsigned(pointer) = value else {
                return Err(unsupported("transport", "schedule pointer"));
            };
            let id = u32::try_from(
                pointer
                    .checked_sub(1)
                    .ok_or_else(|| unsupported("transport", "null schedule pointer"))?,
            )
            .map_err(|_| unsupported("transport", "schedule pointer width"))?;
            if !seen.insert(id) {
                return Err(unsupported("transport", "duplicate scheduled object"));
            }
        }
    }
    if running.iter().copied().collect::<Vec<_>>() != runner.transport.jobs.snapshot().occupied {
        return Err(unsupported("transport", "running job membership"));
    }
    Ok(())
}
