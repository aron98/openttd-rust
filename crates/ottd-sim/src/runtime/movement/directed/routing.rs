//! Directed `RoadFindPathToDest` and YAPF, without orderless random selection.
mod cache;
mod cost;
mod heap;
mod network;
mod search;
use super::{RoadPhases, State, WorldTickError, number, unsupported};
use cost::{Costs, Destination};
use network::{Network, Position, TileKind, reaches};
#[derive(Clone, Copy)]
struct Node {
    start: Position,
    end: Position,
    parent: Option<usize>,
    cost: i64,
    estimate: i64,
    choice: bool,
}
pub(super) fn select(
    state: &mut State<'_>,
    runner: &RoadPhases<'_>,
    entry: (u32, u8),
) -> Result<u8, WorldTickError> {
    let id = runner.id;
    let network = Network::new(state, id)?;
    let tile = network.tile(entry.0)?;
    if matches!(tile.kind, TileKind::Empty) {
        return reverse(entry.1);
    }
    let cargo = super::SavedVehicleView::candidate(state.view(), id)?.cargo_type()?;
    let spec = runner
        .content
        .cargo()
        .get(usize::from(cargo))
        .ok_or_else(|| unsupported("road_yapf", "cargo specification"))?;
    let kind = if spec.classes & 1 != 0 { 3 } else { 2 };
    let permitted = match tile.kind {
        TileKind::Depot(direction) => tile.owner == network.owner && direction != entry.1,
        TileKind::Terminal {
            direction,
            kind: stop_kind,
            status,
            ..
        } => {
            tile.owner == network.owner
                && direction != entry.1
                && stop_kind == kind
                && (state.number(b"PATS", 0, "pf.roadveh_queue")? != 0 || status & 3 != 0)
        }
        TileKind::Empty | TileKind::Road => true,
    };
    let tracks = if permitted {
        tile.tracks & reaches(entry.1)?
    } else {
        0
    };
    let mut cached = cache::read(state, id)?;
    if tracks == 0 {
        cache::write(state, id, &[])?;
        return reverse(entry.1);
    }
    if number(state, id, false, "dest_tile")? == 0 {
        return Err(unsupported("road_yapf", "directed target missing"));
    }
    if tracks.is_power_of_two() {
        if cached.last().is_some_and(|p| p.tile == entry.0) {
            cache::write(state, id, &[])?;
        }
        return u8::try_from(tracks.trailing_zeros())
            .map_err(|_| unsupported("road_yapf", "single track"));
    }
    if let Some(last) = cached.last().copied() {
        if last.tile == entry.0 && last.track < 16 && tracks & (1 << last.track) != 0 {
            cached.pop();
            cache::write(state, id, &cached)?;
            return Ok(last.track);
        }
        cached.clear();
    }
    let destination = destination(&network, id, kind)?;
    let costs = Costs {
        network,
        destination,
        curve: state.number(b"PATS", 0, "pf.yapf.road_curve_penalty")?,
        occupied: state.number(b"PATS", 0, "pf.yapf.road_stop_bay_occupied_penalty")?,
    };
    let limit = usize::try_from(state.number(b"PATS", 0, "pf.yapf.max_search_nodes")?)
        .map_err(|_| unsupported("road_yapf", "search limit"))?;
    let path = search::find(&costs, (entry.0, tracks), limit)?
        .ok_or_else(|| unsupported("road_yapf", "no best path and lost-vehicle lifecycle"))?;
    if !path.found {
        return Err(unsupported("road_yapf", "lost-vehicle lifecycle"));
    }
    cache::write(state, id, &path.cache)?;
    Ok(path.track)
}
fn reverse(direction: u8) -> Result<u8, WorldTickError> {
    [6, 7, 14, 15]
        .get(usize::from(direction))
        .copied()
        .ok_or_else(|| unsupported("road_yapf", "reverse direction"))
}
pub(super) fn next(
    state: &State<'_>,
    runner: &RoadPhases<'_>,
    direction: u8,
) -> Result<u32, WorldTickError> {
    let vehicle = super::SavedVehicleView::candidate(state.view(), runner.id)?;
    Network::new(state, runner.id)?.next(vehicle.tile()?, direction)
}
pub(super) fn physical_tile(state: &State<'_>, id: super::VehicleId) -> Result<(), WorldTickError> {
    let tile = super::SavedVehicleView::candidate(state.view(), id)?.tile()?;
    match Network::new(state, id)?.tile(tile)?.kind {
        TileKind::Road => Ok(()),
        TileKind::Depot(direction) => {
            let track = u8::try_from(number(state, id, true, "state")?)
                .map_err(|_| unsupported("road_route", "depot track"))?;
            if (Position { tile, track }).exit()? != direction {
                return Err(unsupported("road_route", "depot arrival lifecycle"));
            }
            Ok(())
        }
        TileKind::Terminal { .. } => Err(unsupported("road_route", "terminal entry lifecycle")),
        TileKind::Empty => Err(unsupported("road_route", "vehicle outside road")),
    }
}

fn destination(
    network: &Network<'_, '_>,
    id: super::VehicleId,
    kind: u8,
) -> Result<Destination, WorldTickError> {
    let state = network.state;
    let target = u32::try_from(number(state, id, false, "dest_tile")?)
        .map_err(|_| unsupported("road_yapf", "target tile"))?;
    let (stop_id, stop) = network.stop(target)?;
    let station = u16::try_from(number(state, id, false, "current_order.dest")?)
        .map_err(|_| unsupported("road_yapf", "target station"))?;
    let target_map = state.view().tile(target)?;
    let table = state.table(b"STNN")?;
    let normal = crate::runtime::stations::saved::StationRow {
        schema: table.schema(),
        record: table
            .record(u32::from(station))
            .ok_or_else(|| unsupported("road_yapf", "target station"))?,
    }
    .child("normal")?;
    let first = normal.number(if kind == 3 {
        "bus_stops"
    } else {
        "truck_stops"
    })?;
    if stop.number("next")? != 0
        || first != u64::from(stop_id).wrapping_add(1)
        || target_map.m2() != station
        || (target_map.m6() >> 3) & 15 != kind
    {
        return Err(unsupported(
            "road_yapf",
            "multiple target stops and cache trimming",
        ));
    }
    Ok(Destination {
        station,
        kind,
        tile: target,
    })
}
