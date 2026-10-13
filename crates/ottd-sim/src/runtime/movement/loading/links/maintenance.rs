//! Native station link expiry and age-based graph compression for empty manual cargo.
use super::{State, WireValue, WorldEdit, WorldTickError, distance, records, unsupported};
use crate::runtime::stations::saved::StationRow;
use ottd_save::{TableRecord, TableSchema, world::PathElement};

pub(in crate::runtime::movement) fn station(
    state: &mut State<'_>,
    station: u32,
) -> Result<(), WorldTickError> {
    if state
        .number(b"DATE", 0, "tick_counter")?
        .wrapping_add(i64::from(station))
        % 504
        != 0
    {
        return Ok(());
    }
    for cargo in 0..64_u8 {
        let goods = records::goods(state, station, cargo)?;
        let graph = goods.number("link_graph")?;
        if graph == 65535 {
            continue;
        }
        let graph =
            u32::try_from(graph).map_err(|_| unsupported("link_maintenance", "graph ID"))?;
        let source = usize::try_from(goods.number("node")?)
            .map_err(|_| unsupported("link_maintenance", "source node"))?;
        expire(state, (station, graph), source)?;
        let date = state.number(b"DATE", 0, "economy_date")?;
        if date.wrapping_sub(state.number(b"LGRP", graph, "last_compression")?) > 256 {
            compress(state, graph, date)?;
        }
    }
    Ok(())
}
fn signed(row: StationRow<'_>, name: &str) -> Result<i64, WorldTickError> {
    State::number_value(crate::world_access::row_field(
        row.schema, row.record, name,
    )?)
}
fn expire(
    state: &mut State<'_>,
    station_graph: (u32, u32),
    source: usize,
) -> Result<(), WorldTickError> {
    let (station, graph) = station_graph;
    let (schema, nodes) = records::row(state, *b"LGRP", graph)?.rows("nodes")?;
    let row = StationRow {
        schema,
        record: nodes
            .get(source)
            .ok_or_else(|| unsupported("link_maintenance", "source node"))?,
    };
    let (edge_schema, edges) = row.rows("edges")?;
    let mut retained = Vec::with_capacity(edges.len());
    let date = state.number(b"DATE", 0, "economy_date")?;
    for record in edges {
        let edge = StationRow {
            schema: edge_schema,
            record,
        };
        let target = usize::try_from(edge.number("dest_node")?)
            .map_err(|_| unsupported("link_maintenance", "destination node"))?;
        let destination = u32::try_from(
            (StationRow {
                schema,
                record: nodes
                    .get(target)
                    .ok_or_else(|| unsupported("link_maintenance", "destination node"))?,
            })
            .number("station")?,
        )
        .map_err(|_| unsupported("link_maintenance", "station ID"))?;
        let timeout = 32_i64.wrapping_add(i64::from(distance(state, station, destination)? >> 3));
        let unrestricted = signed(edge, "last_unrestricted_update")?;
        let restricted = signed(edge, "last_restricted_update")?;
        if date < unrestricted.max(restricted) {
            return Err(unsupported("link_maintenance", "future link timestamp"));
        }
        if date.wrapping_sub(unrestricted.max(restricted)) > timeout {
            continue;
        }
        let mut record = record.clone();
        if unrestricted != -1 && date.wrapping_sub(unrestricted) > timeout {
            records::edit(
                edge_schema,
                &mut record,
                ("last_unrestricted_update", WireValue::Signed(-1)),
            )?;
        } else if restricted != -1 && date.wrapping_sub(restricted) > timeout {
            records::edit(
                edge_schema,
                &mut record,
                ("last_restricted_update", WireValue::Signed(-1)),
            )?;
        }
        retained.push(record);
    }
    state.apply(WorldEdit::StructList {
        chunk: *b"LGRP",
        record: graph,
        path: vec![
            PathElement::Field("nodes".into()),
            PathElement::Index(source),
            PathElement::Field("edges".into()),
        ],
        rows: retained,
    })
}
fn compress(state: &mut State<'_>, graph: u32, date: i64) -> Result<(), WorldTickError> {
    let row = records::row(state, *b"LGRP", graph)?;
    let last = signed(row, "last_compression")?;
    let (schema, nodes) = row.rows("nodes")?;
    let mut nodes = nodes.to_vec();
    for node in &mut nodes {
        let row = StationRow {
            schema,
            record: node,
        };
        let supply = row.number("supply")? / 2;
        let (edge_schema, edges) = row.rows("edges")?;
        let mut edges = edges.to_vec();
        for edge in &mut edges {
            compress_edge(edge_schema, edge)?;
        }
        records::edit(schema, node, ("supply", WireValue::Unsigned(supply)))?;
        records::edit(schema, node, ("edges", WireValue::Structs(edges)))?;
    }
    state.set(b"LGRP", graph, "nodes", WireValue::Structs(nodes))?;
    state.set_number(
        b"LGRP",
        graph,
        "last_compression",
        date.wrapping_add(last) / 2,
    )
}
fn compress_edge(schema: &TableSchema, record: &mut TableRecord) -> Result<(), WorldTickError> {
    let row = StationRow { schema, record };
    let old = row.number("capacity")?;
    if old == 0 {
        return Ok(());
    }
    let capacity = (old / 2).max(1);
    let sum = row.number("travel_time_sum")?;
    let sum = if old < 65536 {
        sum.wrapping_mul(capacity)
            .checked_div(old)
            .ok_or_else(|| unsupported("link_compression", "zero edge capacity"))?
    } else if sum != 0 {
        (sum / 2).max(1)
    } else {
        0
    };
    let usage = row.number("usage")? / 2;
    for (name, value) in [
        ("capacity", capacity),
        ("usage", usage),
        ("travel_time_sum", sum),
    ] {
        records::edit(schema, record, (name, WireValue::Unsigned(value)))?;
    }
    Ok(())
}
