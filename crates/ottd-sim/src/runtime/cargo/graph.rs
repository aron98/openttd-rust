//! Native produced supply creates a graph when needed and updates its source node.
use super::{RuntimeError, read::Read, write};
use crate::{
    WorldTickError, WorldTickState as State, runtime::transport::TransportRuntime,
    unsupported_tick as unsupported,
};
use ottd_save::{
    TableRecord, TableSchema, WireValue,
    world::{PathElement, WorldEdit},
};

fn record(
    schema: &TableSchema,
    fields: &[(&str, WireValue)],
) -> Result<TableRecord, WorldTickError> {
    schema
        .fields()
        .iter()
        .map(|field| {
            fields
                .iter()
                .find(|(name, _)| *name == field.name())
                .map(|(_, value)| value.clone())
                .ok_or_else(|| unsupported("cargo_graph", "constructor field"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(TableRecord::new)
}
pub(super) fn supply(
    state: &mut State<'_>,
    transport: &mut TransportRuntime,
    station: u32,
    cargo: u8,
    amount: u32,
) -> Result<(), WorldTickError> {
    let graph = write::goods(state, station, cargo)?.number("link_graph")?;
    let graph = if graph == 65535 {
        if !transport.graphs.can_allocate(1) {
            return Ok(());
        }
        let graph = transport.graphs.allocate().map_err(RuntimeError::from)?;
        let value = record(
            state.table(b"LGRP")?.schema(),
            &[
                ("cargo", WireValue::Unsigned(u64::from(cargo))),
                (
                    "last_compression",
                    WireValue::Signed(state.number(b"DATE", 0, "economy_date")?),
                ),
                ("nodes", WireValue::Structs(Vec::new())),
            ],
        )?;
        state.apply(WorldEdit::InsertRecord {
            chunk: *b"LGRP",
            record: graph,
            value,
        })?;
        let WireValue::Array(schedule) = state.value(b"LGRS", 0, "schedule")? else {
            return Err(unsupported("cargo_graph", "schedule"));
        };
        let mut schedule = schedule.clone();
        schedule.push(WireValue::Unsigned(u64::from(graph) + 1));
        state.apply(WorldEdit::ArrayList {
            chunk: *b"LGRS",
            record: 0,
            path: vec![PathElement::Field("schedule".into())],
            values: schedule,
        })?;
        let (schema, _) = Read::Candidate(state.view())
            .row(*b"LGRP", graph)?
            .rows("nodes")?;
        let xy = Read::Candidate(state.view())
            .row(*b"STNN", station)?
            .child("normal")?
            .child("base")?
            .number("xy")?;
        let value = record(
            schema,
            &[
                ("station", WireValue::Unsigned(u64::from(station))),
                ("xy", WireValue::Unsigned(xy)),
                ("supply", WireValue::Unsigned(0)),
                (
                    "demand",
                    WireValue::Unsigned(write::goods(state, station, cargo)?.number("status")? & 1),
                ),
                ("last_update", WireValue::Signed(-1)),
                ("edges", WireValue::Structs(Vec::new())),
            ],
        )?;
        state.set(b"LGRP", graph, "nodes", WireValue::Structs(vec![value]))?;
        write::goods_field(state, station, cargo, "link_graph", u64::from(graph))?;
        write::goods_field(state, station, cargo, "node", 0)?;
        graph
    } else {
        u32::try_from(graph).map_err(|_| unsupported("cargo_graph", "graph id"))?
    };
    let node = usize::try_from(write::goods(state, station, cargo)?.number("node")?)
        .map_err(|_| unsupported("cargo_graph", "node id"))?;
    let row = Read::Candidate(state.view()).row(*b"LGRP", graph)?;
    if row.number("cargo")? != u64::from(cargo) {
        return Err(unsupported("cargo_graph", "cargo identity"));
    }
    let (schema, nodes) = row.rows("nodes")?;
    let node_row = crate::runtime::stations::saved::StationRow {
        schema,
        record: nodes
            .get(node)
            .ok_or_else(|| unsupported("cargo_graph", "node"))?,
    };
    if node_row.number("station")? != u64::from(station) {
        return Err(unsupported("cargo_graph", "station identity"));
    }
    let supply = u32::try_from(node_row.number("supply")?)
        .map_err(|_| unsupported("cargo_graph", "supply"))?
        .wrapping_add(amount);
    for (name, value) in [
        ("supply", WireValue::Unsigned(u64::from(supply))),
        (
            "last_update",
            WireValue::Signed(state.number(b"DATE", 0, "economy_date")?),
        ),
    ] {
        state.apply(WorldEdit::Field {
            chunk: *b"LGRP",
            record: graph,
            path: vec![
                PathElement::Field("nodes".into()),
                PathElement::Index(node),
                PathElement::Field(name.into()),
            ],
            value,
        })?;
    }
    Ok(())
}
