//! Reconstruct saved packet ownership and the native per-list afterload caches.
use super::{CargoRuntime, RuntimeError, StationList, VehicleList, recipients};
use crate::runtime::{
    pools::PoolAllocator,
    stations::saved::{self, StationRow},
};
use crate::world_access::row_field;
use ottd_save::{
    WireValue,
    world::{Pool, World},
};
use std::collections::BTreeMap;

pub(super) fn restore(world: &World) -> Result<CargoRuntime, RuntimeError> {
    let packets = world
        .tables()
        .get(b"CAPA")
        .ok_or(RuntimeError::Invalid("CAPA"))?;
    let (town_near, industry_near) = recipients::restore(world)?;
    let mut result = CargoRuntime {
        pool: PoolAllocator::restore(0xff_f000, 1024, packets.records().keys().copied())?,
        vehicles: BTreeMap::new(),
        stations: station_lists(world)?,
        town_near,
        industry_near,
    };
    for list in &world.derived().cargo_lists {
        match list.owner.pool {
            Pool::Vehicle => {
                let actions = actions(world, list.owner.id)?;
                if actions.iter().try_fold(0_u32, |a, b| a.checked_add(*b)) != Some(list.count) {
                    return Err(RuntimeError::Invalid("cargo action count total"));
                }
                result.vehicles.insert(
                    list.owner.id,
                    VehicleList {
                        packets: list.packets.clone(),
                        count: list.count,
                        periods: list.periods_in_transit,
                        feeder_share: list.feeder_share,
                        actions,
                    },
                );
            }
            Pool::Station => {
                let cargo = list
                    .cargo_type
                    .ok_or(RuntimeError::Invalid("station cargo type"))?;
                let next = list
                    .next_hop
                    .ok_or(RuntimeError::Invalid("station cargo destination"))?;
                let cache = result
                    .stations
                    .get_mut(&list.owner.id)
                    .and_then(|goods| goods.get_mut(&cargo))
                    .ok_or(RuntimeError::Invalid("station cargo cache"))?;
                cache.packets.insert(next, list.packets.clone());
                cache.count = cache.count.wrapping_add(list.count);
                cache.periods = cache.periods.wrapping_add(list.periods_in_transit);
            }
            Pool::Town | Pool::Industry => return Err(RuntimeError::Invalid("cargo list owner")),
        }
    }
    Ok(result)
}

fn station_lists(world: &World) -> Result<BTreeMap<u32, BTreeMap<u32, StationList>>, RuntimeError> {
    let table = world
        .tables()
        .get(b"STNN")
        .ok_or(RuntimeError::Invalid("STNN"))?;
    let mut stations = BTreeMap::new();
    for id in table.records().keys() {
        let station = saved::row(world, *b"STNN", *id)?;
        let (_, normal) = station.rows("normal")?;
        if normal.is_empty() {
            continue;
        }
        let normal = station.child("normal")?;
        let (schema, goods) = normal.rows("goods")?;
        let mut caches = BTreeMap::new();
        for (cargo, record) in goods.iter().enumerate() {
            let row = StationRow { schema, record };
            let reserved = u32::try_from(row.number("cargo.reserved_count")?)
                .map_err(|_| RuntimeError::Invalid("reserved cargo count"))?;
            if reserved != 0 || !row.rows("cargo")?.1.is_empty() || !row.rows("flow")?.1.is_empty()
            {
                caches.insert(
                    u32::try_from(cargo).map_err(|_| RuntimeError::Invalid("cargo type"))?,
                    StationList {
                        reserved,
                        ..StationList::default()
                    },
                );
            }
        }
        stations.insert(*id, caches);
    }
    Ok(stations)
}

fn actions(world: &World, id: u32) -> Result<[u32; 4], RuntimeError> {
    let root = saved::row(world, *b"VEHS", id)?;
    for kind in ["train", "roadveh", "ship", "aircraft"] {
        if root.rows(kind)?.1.is_empty() {
            continue;
        }
        let common = root.child(kind)?.child("common")?;
        let WireValue::Array(values) =
            row_field(common.schema, common.record, "cargo.action_counts")
                .map_err(|_| RuntimeError::Invalid("vehicle cargo actions"))?
        else {
            return Err(RuntimeError::Invalid("vehicle cargo actions"));
        };
        let mut actions = [0; 4];
        if values.len() != actions.len() {
            return Err(RuntimeError::Invalid("vehicle cargo action count"));
        }
        for (target, value) in actions.iter_mut().zip(values) {
            let WireValue::Unsigned(value) = value else {
                return Err(RuntimeError::Invalid("vehicle cargo action"));
            };
            *target = u32::try_from(*value)
                .map_err(|_| RuntimeError::Invalid("vehicle cargo action range"))?;
        }
        return Ok(actions);
    }
    Err(RuntimeError::Invalid("vehicle cargo owner type"))
}
