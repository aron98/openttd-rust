#![expect(
    clippy::redundant_pub_crate,
    reason = "station transaction adapters are shared internally with commands"
)]
pub(crate) mod acceptance;
mod acceptance_data;
mod geometry;
#[cfg(test)]
mod native;
mod publication;
#[cfg(test)]
mod tests;
pub(crate) use publication::{StationContext, StationPublication};
pub(crate) mod saved;
use super::{RuntimeError, pools::PoolAllocator};
pub use geometry::{RoadStationGeometry, StationArea};
use ottd_save::world::World;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StationRuntime {
    pub stations: PoolAllocator,
    pub stops: PoolAllocator,
    pub geometry: BTreeMap<u32, RoadStationGeometry>,
    pub infrastructure: BTreeMap<u8, u32>,
}
impl StationRuntime {
    pub(super) fn restore(world: &World) -> Result<Self, RuntimeError> {
        let stations = world
            .tables()
            .get(b"STNN")
            .ok_or(RuntimeError::Invalid("STNN"))?;
        let stops = world
            .tables()
            .get(b"ROAD")
            .ok_or(RuntimeError::Invalid("ROAD"))?;
        let mut result = Self {
            stations: PoolAllocator::restore(64000, 32, stations.records().keys().copied())?,
            stops: PoolAllocator::restore(64000, 32, stops.records().keys().copied())?,
            geometry: BTreeMap::new(),
            infrastructure: BTreeMap::new(),
        };
        let radius = catchment_radius(world)?;
        for id in stations.records().keys() {
            let row = saved::row(world, *b"STNN", *id)?;
            let facilities = row.number("facilities")?;
            if facilities & !6 != 0 {
                continue;
            }
            let geometry = restore_geometry(
                saved::StationRead::Committed(world),
                *id,
                (world.map().width(), world.map().height()),
                radius,
            )?;
            if geometry.station.is_some_and(|area| area.left == 0) {
                continue;
            }
            result.geometry.insert(*id, geometry);
        }
        let companies = world
            .tables()
            .get(b"PLYR")
            .ok_or(RuntimeError::Invalid("PLYR"))?;
        for id in companies.records().keys() {
            result.infrastructure.insert(
                u8::try_from(*id).map_err(|_| RuntimeError::Invalid("company ID"))?,
                0,
            );
        }
        for tile in world.map().tiles() {
            if tile.tile_type() >> 4 != 5 || matches!((tile.m6() >> 3) & 15, 1 | 6) {
                continue;
            }
            if let Some(count) = result.infrastructure.get_mut(&(tile.m1() & 31)) {
                *count = count
                    .checked_add(1)
                    .ok_or(RuntimeError::Invalid("station infrastructure overflow"))?;
            }
        }
        Ok(result)
    }
}
pub(crate) fn catchment_radius(world: &World) -> Result<u32, RuntimeError> {
    Ok(
        if crate::world_access::unsigned(world, b"PATS", 0, "station.modified_catchment")
            .map_err(|_| RuntimeError::Invalid("modified catchment setting"))?
            == 0
        {
            4
        } else {
            3
        },
    )
}
fn restore_geometry(
    view: saved::StationRead<'_>,
    station: u32,
    dimensions: (u32, u32),
    radius: u32,
) -> Result<RoadStationGeometry, RuntimeError> {
    let normal = view.row(*b"STNN", station)?.child("normal")?;
    let mut geometry = RoadStationGeometry::default();
    let mut seen = BTreeSet::new();
    for (name, bus) in [("bus_stops", true), ("truck_stops", false)] {
        let mut pointer = normal.number(name)?;
        while pointer != 0 {
            let id = u32::try_from(pointer.saturating_sub(1))
                .map_err(|_| RuntimeError::Invalid("road stop pointer"))?;
            if !seen.insert(id) {
                return Err(RuntimeError::Invalid("road stop chain cycle"));
            }
            let row = view.row(*b"ROAD", id)?;
            let index = u32::try_from(row.number("xy")?)
                .map_err(|_| RuntimeError::Invalid("road stop tile"))?;
            let tile = view.tile(index)?;
            if tile.tile_type() >> 4 != 5
                || u32::from(tile.m2()) != station
                || (tile.m6() >> 3) & 15 != if bus { 3 } else { 2 }
            {
                return Err(RuntimeError::Invalid("road stop chain map membership"));
            }
            geometry.add(index, bus, dimensions, radius)?;
            pointer = row.number("next")?;
        }
    }
    Ok(geometry)
}
