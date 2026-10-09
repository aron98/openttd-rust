use super::{RuntimeError, pools::PoolAllocator};
use ottd_save::{TileState, world::World};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DepotRuntime {
    pub(super) pool: PoolAllocator,
    pub(super) road: BTreeMap<u8, [u32; 63]>,
}

impl DepotRuntime {
    pub(super) fn restore(world: &World) -> Result<Self, RuntimeError> {
        let depots = world
            .tables()
            .get(b"DEPT")
            .ok_or(RuntimeError::Invalid("DEPT"))?;
        let pool = PoolAllocator::restore(64000, 64, depots.records().keys().copied())?;
        let companies = world
            .tables()
            .get(b"PLYR")
            .ok_or(RuntimeError::Invalid("PLYR"))?;
        let mut road = BTreeMap::new();
        for id in companies.records().keys() {
            let company = u8::try_from(*id).map_err(|_| RuntimeError::Invalid("company ID"))?;
            road.insert(company, [0; 63]);
        }
        accumulate(world.map().tiles(), world.map().width(), &mut road)?;
        Ok(Self { pool, road })
    }
}

fn accumulate(
    tiles: &[TileState],
    width: u32,
    road: &mut BTreeMap<u8, [u32; 63]>,
) -> Result<(), RuntimeError> {
    let width = usize::try_from(width).map_err(|_| RuntimeError::Invalid("map width"))?;
    if width == 0 || tiles.len().checked_rem(width) != Some(0) {
        return Err(RuntimeError::Invalid("map dimensions"));
    }
    for (index, tile) in tiles.iter().enumerate() {
        let kind = tile.tile_type() >> 4;
        let normal = kind == 2 && tile.m5() >> 6 == 0;
        let depot = kind == 2 && tile.m5() >> 6 == 2;
        let pieces = match kind {
            2 => 2,
            5 if matches!((tile.m6() >> 3) & 15, 2 | 3 | 8) => 2,
            9 if (tile.m5() >> 2) & 3 == 1 => {
                let end = other_end(tiles, width, index)?;
                if index >= end {
                    continue;
                }
                let distance = if tile.m5() & 1 == 0 {
                    index.abs_diff(end)
                } else {
                    index
                        .abs_diff(end)
                        .checked_div(width)
                        .ok_or(RuntimeError::Invalid("map width"))?
                };
                distance
                    .checked_add(1)
                    .and_then(|value| u32::try_from(value).ok())
                    .and_then(|length| length.checked_mul(8))
                    .ok_or(RuntimeError::Invalid("road structure length"))?
            }
            _ => continue,
        };
        for tram in [false, true] {
            let road_type = if tram {
                (tile.m8() >> 6) & 63
            } else {
                u16::from(tile.m4() & 63)
            };
            if road_type == 63 {
                continue;
            }
            let owner = if depot {
                tile.m1() & 31
            } else if tram {
                let owner = tile.m3() >> 4;
                if owner == 15 { 16 } else { owner }
            } else if normal {
                tile.m1() & 31
            } else {
                tile.m7() & 31
            };
            let count = if normal {
                (if tram { tile.m3() } else { tile.m5() } & 15).count_ones()
            } else {
                pieces
            };
            if let Some(company) = road.get_mut(&owner) {
                let total = company
                    .get_mut(usize::from(road_type))
                    .ok_or(RuntimeError::Invalid("road type"))?;
                *total = total
                    .checked_add(count)
                    .ok_or(RuntimeError::Invalid("road infrastructure overflow"))?;
            }
        }
    }
    Ok(())
}

fn other_end(tiles: &[TileState], width: usize, start: usize) -> Result<usize, RuntimeError> {
    let origin = tiles
        .get(start)
        .ok_or(RuntimeError::Invalid("structure start"))?;
    let direction = origin.m5() & 3;
    let bridge = origin.m5() & 128 != 0;
    let height = |index: usize| -> Result<u8, RuntimeError> {
        crate::terrain::tile_minimum_height(
            tiles,
            u32::try_from(width).map_err(|_| RuntimeError::Invalid("map width"))?,
            u32::try_from(index).map_err(|_| RuntimeError::Invalid("tile index"))?,
        )
        .map_err(|_| RuntimeError::Invalid("tile corners"))
    };
    let bottom = if bridge { 0 } else { height(start)? };
    let mut index = start;
    loop {
        let next = match direction {
            0 if index.checked_rem(width).is_some_and(|x| x != 0) => index.checked_sub(1),
            1 => index.checked_add(width),
            2 if index
                .checked_rem(width)
                .and_then(|x| x.checked_add(1))
                .is_some_and(|x| x < width) =>
            {
                index.checked_add(1)
            }
            3 => index.checked_sub(width),
            _ => None,
        }
        .ok_or(RuntimeError::Invalid("unterminated road structure"))?;
        let tile = tiles
            .get(next)
            .ok_or(RuntimeError::Invalid("unterminated road structure"))?;
        if tile.tile_type() >> 4 == 9
            && (tile.m5() & 128 != 0) == bridge
            && tile.m5() & 3 == direction ^ 2
            && (bridge || height(next)? == bottom)
        {
            return Ok(next);
        }
        index = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ottd_save::TileRawParts;

    #[test]
    fn restores_original_depot_ids_from_borrowed_world() -> Result<(), Box<dyn std::error::Error>> {
        let save = ottd_save::Savegame::decode(
            include_bytes!("../../../../fixtures/world/populated-v362.sav"),
            ottd_save::DEFAULT_MAX_BYTES,
        )?;
        let world = World::decode(&save)?;
        let runtime = DepotRuntime::restore(&world)?;
        let ids: Vec<_> = world
            .tables()
            .get(b"DEPT")
            .ok_or("DEPT")?
            .records()
            .keys()
            .copied()
            .collect();
        assert_eq!(runtime.pool.snapshot().occupied, ids);
        assert_eq!(
            runtime.road.len(),
            world.tables().get(b"PLYR").ok_or("PLYR")?.records().len()
        );
        Ok(())
    }

    fn tile(kind: u8, m5: u8, owner: u8) -> TileState {
        TileRawParts {
            tile_type: kind << 4,
            height: 0,
            m1: owner,
            m2: 0,
            m3: 1 << 4 | 3,
            m4: 0,
            m5,
            m6: 0,
            m7: owner,
            m8: 1 << 6,
        }
        .into()
    }

    #[test]
    fn counts_each_surface_and_its_distinct_owners() -> Result<(), RuntimeError> {
        let mut stop = TileRawParts::from(&tile(5, 0, 0));
        stop.m6 = 2 << 3;
        let tiles = [tile(2, 7, 0), tile(2, 64, 0), tile(2, 128, 0), stop.into()];
        let mut road = BTreeMap::from([(0, [0; 63]), (1, [0; 63])]);
        accumulate(&tiles, 4, &mut road)?;
        assert_eq!(
            road.get(&0)
                .ok_or(RuntimeError::Invalid("test company"))?
                .first()
                .copied(),
            Some(9)
        );
        assert_eq!(
            road.get(&0)
                .ok_or(RuntimeError::Invalid("test company"))?
                .get(1)
                .copied(),
            Some(2)
        );
        assert_eq!(
            road.get(&1)
                .ok_or(RuntimeError::Invalid("test company"))?
                .get(1)
                .copied(),
            Some(6)
        );
        Ok(())
    }

    #[test]
    fn bridge_counts_both_ramps_once_and_only_northern_owners() -> Result<(), RuntimeError> {
        let tiles = [tile(9, 128 | 4 | 2, 0), tile(0, 0, 0), tile(9, 128 | 4, 1)];
        let mut road = BTreeMap::from([(0, [0; 63]), (1, [0; 63])]);
        accumulate(&tiles, 3, &mut road)?;
        assert_eq!(
            road.get(&0)
                .ok_or(RuntimeError::Invalid("test company"))?
                .first()
                .copied(),
            Some(24)
        );
        assert_eq!(
            road.get(&1)
                .ok_or(RuntimeError::Invalid("test company"))?
                .first()
                .copied(),
            Some(0)
        );
        assert_eq!(
            road.get(&1)
                .ok_or(RuntimeError::Invalid("test company"))?
                .get(1)
                .copied(),
            Some(24)
        );
        Ok(())
    }

    #[test]
    fn tunnel_matches_bottom_height_not_north_corner() -> Result<(), RuntimeError> {
        let mut high = TileRawParts::from(&tile(9, 4, 0));
        high.height = 1;
        let tiles = [
            tile(9, 6, 0),
            tile(0, 0, 0),
            high.into(),
            tile(0, 0, 0),
            tile(0, 0, 0),
            tile(0, 0, 0),
            tile(0, 0, 0),
            tile(0, 0, 0),
        ];
        assert_eq!(other_end(&tiles, 4, 0)?, 2);
        assert_eq!(other_end(&tiles, 4, 2)?, 0);
        Ok(())
    }

    #[test]
    fn tunnel_skips_opposite_mouth_at_another_height() -> Result<(), RuntimeError> {
        let mut high = TileRawParts::from(&tile(9, 4, 0));
        high.height = 1;
        let mut raised = TileRawParts::from(&tile(0, 0, 0));
        raised.height = 1;
        let tiles = [
            tile(9, 6, 0),
            high.into(),
            raised.into(),
            tile(9, 4, 0),
            tile(0, 0, 0),
            raised.into(),
            raised.into(),
            tile(0, 0, 0),
        ];
        assert_eq!(other_end(&tiles, 4, 0)?, 3);
        assert!(other_end(&tiles, 4, 1).is_err());
        Ok(())
    }

    #[test]
    fn malformed_endpoint_cannot_wrap_to_next_row() {
        let tiles = [
            tile(0, 0, 0),
            tile(9, 128 | 4 | 2, 0),
            tile(9, 128 | 4, 0),
            tile(0, 0, 0),
        ];
        assert!(other_end(&tiles, 2, 1).is_err());
    }
}
