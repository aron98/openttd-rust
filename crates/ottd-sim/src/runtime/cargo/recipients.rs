//! Reciprocal recipient membership for supported vanilla terminal catchments.
use super::{Nearby, RuntimeError, read::Read};
use crate::runtime::stations::{RoadStationGeometry, StationRuntime};
use ottd_save::world::World;
use std::collections::BTreeMap;

pub(super) fn restore(world: &World) -> Result<(Option<Nearby>, Option<Nearby>), RuntimeError> {
    from_view(
        Read::World(world),
        &StationRuntime::restore(world)?.geometry,
    )
}

pub(super) fn from_view(
    view: Read<'_>,
    geometry: &BTreeMap<u32, RoadStationGeometry>,
) -> Result<(Option<Nearby>, Option<Nearby>), RuntimeError> {
    for id in view.ids(*b"STNN")? {
        let station = view.row(*b"STNN", id)?;
        if station.number("facilities")? & !6 != 0
            || !station.rows("speclist")?.1.is_empty()
            || !station.rows("roadstopspeclist")?.1.is_empty()
        {
            return Ok((None, None));
        }
    }
    if view.number(*b"PATS", 0, "station.serve_neutral_industries")? == 0 {
        for id in empty(view, *b"INDY")?.keys() {
            if view.row(*b"INDY", *id)?.number("neutral_station")? != 0 {
                return Ok((None, None));
            }
        }
    }
    let mut towns = empty(view, *b"CITY")?;
    let mut industries = empty(view, *b"INDY")?;
    for (id, station) in geometry {
        for index in &station.catchment {
            let tile = view.tile(*index)?;
            let list = match tile.tile_type() >> 4 {
                3 => towns.get_mut(&u32::from(tile.m2())),
                8 => industries.get_mut(&u32::from(tile.m2())),
                _ => None,
            };
            if let Some(ids) = list {
                if ids.last() != Some(id) {
                    ids.push(*id);
                }
            }
        }
    }
    Ok((Some(towns), Some(industries)))
}

fn empty(view: Read<'_>, chunk: [u8; 4]) -> Result<Nearby, RuntimeError> {
    Ok(view
        .ids(chunk)?
        .into_iter()
        .map(|id| (id, Vec::new()))
        .collect())
}
