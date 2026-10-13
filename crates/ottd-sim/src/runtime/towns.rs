// SPDX-License-Identifier: GPL-2.0-only
//! Vanilla counts and zone radii from `RebuildTownCaches` and `UpdateTownRadius`.
#![expect(
    clippy::redundant_pub_crate,
    reason = "town cache adapters are shared internally with world callbacks"
)]
use super::RuntimeError;
use crate::content::HouseSpec;
use ottd_save::world::World;
use serde::Serialize;

/// Simulation-relevant per-town house caches, in original house-ID order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TownHouseCache {
    /// Native sparse town pool identity.
    pub id: u16,
    /// Completed house population.
    pub population: u32,
    /// Building count, counting a multi-tile house once.
    pub num_houses: u32,
    /// Squared zone radii from outer edge to town centre.
    pub zone_radii: [u32; 5],
    /// Native per-town uint16 counts, including every footprint part.
    pub id_count: Vec<u16>,
    /// Vanilla class-zero sentinel count; class zero is never incremented.
    pub class_count: [u16; 1],
}

/// House/town caches restored independently from saved records and raw map tiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TownHouseCaches {
    /// Native town pool iteration order.
    pub towns: Vec<TownHouseCache>,
    /// Global counts, including every footprint part.
    pub global_id_count: Vec<u32>,
    /// Vanilla class-zero sentinel count.
    pub global_class_count: [u32; 1],
}

impl TownHouseCaches {
    pub(crate) fn restore(world: &World) -> Result<Self, RuntimeError> {
        let towns = world
            .tables()
            .get(b"CITY")
            .ok_or(RuntimeError::Invalid("town pool"))?;
        let mut result = Self {
            towns: towns
                .records()
                .keys()
                .map(|id| {
                    Ok(TownHouseCache {
                        id: u16::try_from(*id).map_err(|_| RuntimeError::Invalid("town ID"))?,
                        population: 0,
                        num_houses: 0,
                        zone_radii: [0; 5],
                        id_count: vec![0; 110],
                        class_count: [0],
                    })
                })
                .collect::<Result<_, RuntimeError>>()?,
            global_id_count: vec![0; 110],
            global_class_count: [0],
        };
        for tile in world.map().tiles() {
            if tile.tile_type() >> 4 != 3 {
                continue;
            }
            let id = tile.m8() & 0x0fff;
            let spec = spec(id)?;
            let town = result.town_mut(tile.m2())?;
            let count = town
                .id_count
                .get_mut(usize::from(id))
                .ok_or(RuntimeError::Invalid("town house count"))?;
            *count = count.wrapping_add(1);
            if tile.m3() & 128 != 0 {
                town.population = town.population.wrapping_add(u32::from(spec.population));
            }
            if north_part(id)?.1 == (0, 0) {
                town.num_houses = town.num_houses.wrapping_add(1);
            }
            let count = result
                .global_id_count
                .get_mut(usize::from(id))
                .ok_or(RuntimeError::Invalid("global house count"))?;
            *count = count.wrapping_add(1);
        }
        for town in &mut result.towns {
            town.zone_radii = radii(town.num_houses)?;
        }
        Ok(result)
    }

    pub(crate) fn town(&self, id: u16) -> Result<&TownHouseCache, RuntimeError> {
        let index = self
            .towns
            .binary_search_by_key(&id, |town| town.id)
            .map_err(|_| RuntimeError::Invalid("house town reference"))?;
        self.towns
            .get(index)
            .ok_or(RuntimeError::Invalid("house town cache"))
    }

    pub(crate) fn town_mut(&mut self, id: u16) -> Result<&mut TownHouseCache, RuntimeError> {
        let index = self
            .towns
            .binary_search_by_key(&id, |town| town.id)
            .map_err(|_| RuntimeError::Invalid("house town reference"))?;
        self.towns
            .get_mut(index)
            .ok_or(RuntimeError::Invalid("house town cache"))
    }
}

pub(crate) fn spec(id: u16) -> Result<HouseSpec, RuntimeError> {
    HouseSpec::vanilla()
        .get(usize::from(id))
        .copied()
        .ok_or(RuntimeError::Unsupported("non-vanilla house ID"))
}

pub(crate) fn north_part(id: u16) -> Result<(u16, (i32, i32)), RuntimeError> {
    if id >= 3 {
        if spec(id.wrapping_sub(1))?.building_flags & 4 != 0 {
            return Ok((id.wrapping_sub(1), (-1, 0)));
        }
        if spec(id.wrapping_sub(1))?.building_flags & 0x18 != 0 {
            return Ok((id.wrapping_sub(1), (0, -1)));
        }
        if spec(id.wrapping_sub(2))?.building_flags & 16 != 0 {
            return Ok((id.wrapping_sub(2), (-1, 0)));
        }
        if spec(id.wrapping_sub(3))?.building_flags & 16 != 0 {
            return Ok((id.wrapping_sub(3), (-1, -1)));
        }
    }
    Ok((id, (0, 0)))
}

fn radii(count: u32) -> Result<[u32; 5], RuntimeError> {
    const TABLE: [[u32; 5]; 23] = [
        [4, 0, 0, 0, 0],
        [16, 0, 0, 0, 0],
        [25, 0, 0, 0, 0],
        [36, 0, 0, 0, 0],
        [49, 0, 4, 0, 0],
        [64, 0, 4, 0, 0],
        [64, 0, 9, 0, 1],
        [64, 0, 9, 0, 4],
        [64, 0, 16, 0, 4],
        [81, 0, 16, 0, 4],
        [81, 0, 16, 0, 4],
        [81, 0, 25, 0, 9],
        [81, 36, 25, 0, 9],
        [81, 36, 25, 16, 9],
        [81, 49, 0, 25, 9],
        [81, 64, 0, 25, 9],
        [81, 64, 0, 36, 9],
        [81, 64, 0, 36, 16],
        [100, 81, 0, 49, 16],
        [100, 81, 0, 49, 25],
        [121, 81, 0, 49, 25],
        [121, 81, 0, 49, 25],
        [121, 81, 0, 49, 36],
    ];
    if count < 92 {
        return TABLE
            .get(usize::try_from(count / 4).map_err(|_| RuntimeError::Invalid("house count"))?)
            .copied()
            .ok_or(RuntimeError::Invalid("town radius"));
    }
    let mass = count / 8;
    Ok([
        mass.wrapping_mul(15).wrapping_sub(40),
        mass.wrapping_mul(9).wrapping_sub(15),
        0,
        mass.wrapping_mul(5).wrapping_sub(5),
        mass.wrapping_mul(3).wrapping_add(5),
    ])
}
