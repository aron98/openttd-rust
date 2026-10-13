//! Destination-first link graph and node allocation from `Station::IncreaseStats`.
use super::{
    RoadPhases, State, Traversal, WireValue, WorldEdit, WorldTickError, records, unsupported,
};
use crate::runtime::RuntimeError;
pub(super) fn connect(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    link: &Traversal,
) -> Result<u32, WorldTickError> {
    let source = records::goods(state, link.source, link.cargo)?.number("link_graph")?;
    let destination = records::goods(state, link.destination, link.cargo)?.number("link_graph")?;
    let graph = match (source, destination) {
        (65535, 65535) => {
            let id = runner
                .transport
                .graphs
                .allocate()
                .map_err(RuntimeError::from)?;
            let record = records::construct(
                state.table(b"LGRP")?.schema(),
                &[
                    ("cargo", WireValue::Unsigned(u64::from(link.cargo))),
                    (
                        "last_compression",
                        WireValue::Signed(state.number(b"DATE", 0, "economy_date")?),
                    ),
                    ("nodes", WireValue::Structs(Vec::new())),
                ],
            )?;
            state.apply(WorldEdit::InsertRecord {
                chunk: *b"LGRP",
                record: id,
                value: record,
            })?;
            let WireValue::Array(schedule) = state.value(b"LGRS", 0, "schedule")? else {
                return Err(unsupported("links", "schedule"));
            };
            let mut schedule = schedule.clone();
            schedule.push(WireValue::Unsigned(u64::from(id).wrapping_add(1)));
            records::schedule(state, "schedule", schedule)?;
            add(state, id, (link.destination, link.cargo))?;
            id
        }
        (65535, id) | (id, 65535) => {
            u32::try_from(id).map_err(|_| unsupported("links", "graph ID"))?
        }
        (a, b) if a == b => u32::try_from(a).map_err(|_| unsupported("links", "graph ID"))?,
        _ => return Err(unsupported("links", "merging separate graphs")),
    };
    if source == 65535 {
        add(state, graph, (link.source, link.cargo))?;
    }
    if source != 65535 && destination == 65535 {
        add(state, graph, (link.destination, link.cargo))?;
    }
    Ok(graph)
}
fn add(state: &mut State<'_>, graph: u32, station_cargo: (u32, u8)) -> Result<(), WorldTickError> {
    let (station, cargo) = station_cargo;
    let row = records::row(state, *b"LGRP", graph)?;
    if row.number("cargo")? != u64::from(cargo) {
        return Err(unsupported("links", "graph cargo"));
    }
    let (schema, nodes) = row.rows("nodes")?;
    let index = u64::try_from(nodes.len()).map_err(|_| unsupported("links", "node count"))?;
    if index >= 65535 {
        return Err(unsupported("links", "node capacity"));
    }
    let xy = records::normal(state, station)?
        .child("base")?
        .number("xy")?;
    let demand = records::goods(state, station, cargo)?.number("status")? & 1;
    let record = records::construct(
        schema,
        &[
            ("station", WireValue::Unsigned(u64::from(station))),
            ("xy", WireValue::Unsigned(xy)),
            ("supply", WireValue::Unsigned(0)),
            ("demand", WireValue::Unsigned(demand)),
            ("last_update", WireValue::Signed(-1)),
            ("edges", WireValue::Structs(Vec::new())),
        ],
    )?;
    let mut nodes = nodes.to_vec();
    nodes.push(record);
    state.set(b"LGRP", graph, "nodes", WireValue::Structs(nodes))?;
    records::goods_field(
        state,
        station_cargo,
        ("link_graph", WireValue::Unsigned(u64::from(graph))),
    )?;
    records::goods_field(state, station_cargo, ("node", WireValue::Unsigned(index)))
}
