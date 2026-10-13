//! Native ordinary-order link refresh and measured traversal updates.
mod edge;
mod maintenance;
pub(in crate::runtime::movement) use maintenance::station as station_links;
mod nodes;
use super::{
    RoadPhases, SavedVehicleView, State, WorldTickError, number, orders, records, unsupported,
};
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};
#[derive(Clone, Copy)]
enum Update {
    Refresh,
    Increase,
}
#[derive(Clone, Copy)]
struct Traversal {
    source: u32,
    destination: u32,
    cargo: u8,
    capacity: u32,
    usage: u32,
    time: u32,
    mode: Update,
}

pub(super) fn refresh(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    let destinations = orders::destinations(state, runner.id)?;
    let (_, start, len) = orders::cursor(state, runner.id)?;
    if number(state, runner.id, false, "cur_implicit_order_index")?
        != i64::try_from(start).map_err(|_| unsupported("link_refresh", "index"))?
    {
        return Err(unsupported("link_refresh", "implicit order decision node"));
    }
    let cargo = SavedVehicleView::candidate(state.view(), runner.id)?.cargo_type()?;
    let capacity = u32::try_from(number(state, runner.id, false, "refit_cap")?)
        .map_err(|_| unsupported("link_refresh", "capacity"))?;
    let speed = u32::from(runner.cache.max_speed / 2);
    if capacity == 0 || speed == 0 {
        return Ok(());
    }
    let count = std::num::NonZeroUsize::new(len)
        .ok_or_else(|| unsupported("link_refresh", "empty order list"))?;
    let speed = std::num::NonZeroU32::new(speed)
        .ok_or_else(|| unsupported("link_refresh", "zero speed"))?;
    for step in 0..len {
        let from = start.wrapping_add(step) % count;
        let to = from.wrapping_add(1) % count;
        let source = *destinations
            .get(from)
            .ok_or_else(|| unsupported("link_refresh", "source order"))?;
        let destination = *destinations
            .get(to)
            .ok_or_else(|| unsupported("link_refresh", "destination order"))?;
        if source == destination {
            continue;
        }
        let distance = distance(state, source, destination)?;
        update(
            state,
            runner,
            Traversal {
                source,
                destination,
                cargo,
                capacity,
                usage: 0,
                time: distance.wrapping_mul(4096) / speed,
                mode: Update::Refresh,
            },
        )?;
    }
    Ok(())
}
pub(super) fn arrival(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    destination: u32,
) -> Result<(), WorldTickError> {
    let source = u32::try_from(number(state, runner.id, false, "last_loading_station")?)
        .map_err(|_| unsupported("link_arrival", "source"))?;
    if source == 65535 || source == destination {
        return Ok(());
    }
    let capacity = u32::try_from(number(state, runner.id, false, "refit_cap")?)
        .map_err(|_| unsupported("link_arrival", "capacity"))?;
    if capacity == 0 {
        return Ok(());
    }
    let time = u32::try_from(
        state
            .number(b"DATE", 0, "tick_counter")?
            .wrapping_sub(number(state, runner.id, false, "last_loading_tick")?)
            & 0xffff_ffff,
    )
    .map_err(|_| unsupported("link_arrival", "travel time"))?;
    let cargo = SavedVehicleView::candidate(state.view(), runner.id)?.cargo_type()?;
    update(
        state,
        runner,
        Traversal {
            source,
            destination,
            cargo,
            capacity,
            usage: capacity
                .min(SavedVehicleView::candidate(state.view(), runner.id)?.stored_count()?),
            time,
            mode: Update::Increase,
        },
    )
}
pub(super) fn distance(
    state: &State<'_>,
    source: u32,
    destination: u32,
) -> Result<u32, WorldTickError> {
    let xy = |station| -> Result<u32, WorldTickError> {
        u32::try_from(
            records::normal(state, station)?
                .child("base")?
                .number("xy")?,
        )
        .map_err(|_| unsupported("links", "station tile"))
    };
    let a = xy(source)?;
    let b = xy(destination)?;
    let width =
        std::num::NonZeroU32::new(state.width).ok_or_else(|| unsupported("links", "map width"))?;
    Ok((a % width)
        .abs_diff(b % width)
        .wrapping_add((a / width).abs_diff(b / width)))
}
fn update(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    link: Traversal,
) -> Result<(), WorldTickError> {
    let graph = nodes::connect(state, runner, &link)?;
    let source = usize::try_from(records::goods(state, link.source, link.cargo)?.number("node")?)
        .map_err(|_| unsupported("links", "source node"))?;
    let destination = records::goods(state, link.destination, link.cargo)?.number("node")?;
    let row = records::row(state, *b"LGRP", graph)?;
    let (node_schema, node_rows) = row.rows("nodes")?;
    let source_row = node_rows
        .get(source)
        .ok_or_else(|| unsupported("links", "missing source node"))?;
    let node = crate::runtime::stations::saved::StationRow {
        schema: node_schema,
        record: source_row,
    };
    let (edge_schema, edges) = node.rows("edges")?;
    let mut edges = edges.to_vec();
    edge::update(
        edge_schema,
        &mut edges,
        edge::Observation {
            destination,
            date: state.number(b"DATE", 0, "economy_date")?,
            link: &link,
        },
    )?;
    state.apply(WorldEdit::StructList {
        chunk: *b"LGRP",
        record: graph,
        path: vec![
            PathElement::Field("nodes".into()),
            PathElement::Index(source),
            PathElement::Field("edges".into()),
        ],
        rows: edges,
    })
}
