//! `CFollowTrackRoad` for reached flat vanilla road/depot/terminal tiles.
use super::super::{SavedVehicleView, State, VehicleId, WorldTickError, unsupported};
use crate::runtime::stations::saved::StationRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Position {
    pub tile: u32,
    pub track: u8,
}
impl Position {
    pub(super) fn exit(self) -> Result<u8, WorldTickError> {
        [0, 1, 0, 1, 2, 1, 0, 0, 2, 3, 3, 2, 3, 0]
            .get(usize::from(self.track))
            .copied()
            .ok_or_else(|| unsupported("road_yapf", "track exit"))
    }
    pub(super) fn key(self) -> Result<(u32, u8), WorldTickError> {
        Ok((self.tile, self.exit()?))
    }
}
pub(super) fn reaches(direction: u8) -> Result<u16, WorldTickError> {
    [0x1009, 0x16, 0x520, 0x2a00]
        .get(usize::from(direction))
        .copied()
        .ok_or_else(|| unsupported("road_yapf", "entry direction"))
}
#[derive(Clone, Copy)]
pub(super) enum TileKind {
    Empty,
    Road,
    Depot(u8),
    Terminal {
        direction: u8,
        station: u16,
        kind: u8,
        status: u8,
    },
}
pub(super) struct Tile {
    pub kind: TileKind,
    pub tracks: u16,
    pub owner: u8,
}
pub(super) struct Network<'a, 'w> {
    pub state: &'a State<'w>,
    pub owner: u8,
}
impl<'a, 'w> Network<'a, 'w> {
    pub(super) fn new(state: &'a State<'w>, id: VehicleId) -> Result<Self, WorldTickError> {
        Ok(Self {
            state,
            owner: SavedVehicleView::candidate(state.view(), id)?.owner()?,
        })
    }
    pub(super) fn next(&self, tile: u32, direction: u8) -> Result<u32, WorldTickError> {
        let offset = match direction {
            0 => -1,
            1 => i64::from(self.state.width),
            2 => 1,
            3 => i64::from(self.state.width).wrapping_neg(),
            _ => return Err(unsupported("road_yapf", "tile direction")),
        };
        u32::try_from(i64::from(tile).wrapping_add(offset))
            .map_err(|_| unsupported("road_yapf", "map edge"))
    }
    pub(super) fn stop(&self, tile: u32) -> Result<(u32, StationRow<'_>), WorldTickError> {
        let map = self.state.view().tile(tile)?;
        let stations = self.state.table(b"STNN")?;
        let station = StationRow {
            schema: stations.schema(),
            record: stations
                .record(u32::from(map.m2()))
                .ok_or_else(|| unsupported("road_yapf", "station"))?,
        }
        .child("normal")?;
        let field = if (map.m6() >> 3) & 15 == 3 {
            "bus_stops"
        } else {
            "truck_stops"
        };
        let mut pointer = station.number(field)?;
        let stops = self.state.table(b"ROAD")?;
        for _ in 0..65536 {
            let id = u32::try_from(
                pointer
                    .checked_sub(1)
                    .ok_or_else(|| unsupported("road_yapf", "stop chain"))?,
            )
            .map_err(|_| unsupported("road_yapf", "stop pointer"))?;
            let row = StationRow {
                schema: stops.schema(),
                record: stops
                    .record(id)
                    .ok_or_else(|| unsupported("road_yapf", "missing stop"))?,
            };
            if row.number("xy")? == u64::from(tile) {
                return Ok((id, row));
            }
            pointer = row.number("next")?;
        }
        Err(unsupported("road_yapf", "cyclic stop chain"))
    }
    pub(super) fn tile(&self, tile: u32) -> Result<Tile, WorldTickError> {
        let map = self.state.view().tile(tile)?;
        let kind = match map.tile_type() >> 4 {
            0 | 3 | 4 | 6 | 7 | 8 | 10 => {
                return Ok(Tile {
                    kind: TileKind::Empty,
                    tracks: 0,
                    owner: map.m1() & 31,
                });
            }
            2 if map.m5() >> 6 == 0 => TileKind::Road,
            2 if map.m5() >> 6 == 2 => TileKind::Depot(map.m5() & 3),
            5 if matches!((map.m6() >> 3) & 15, 2 | 3) && map.m5() < 4 => {
                let (_, row) = self.stop(tile)?;
                TileKind::Terminal {
                    direction: map.m5(),
                    station: map.m2(),
                    kind: (map.m6() >> 3) & 15,
                    status: u8::try_from(row.number("status")?)
                        .map_err(|_| unsupported("road_yapf", "stop status"))?,
                }
            }
            _ => return Err(unsupported("road_yapf", "reached unsupported tile family")),
        };
        if map.m4() & 63 != 0 {
            return Err(unsupported("road_yapf", "non-vanilla road type"));
        }
        for offset in [1, self.state.width, self.state.width.wrapping_add(1)] {
            if self.state.view().tile(tile.wrapping_add(offset))?.height() != map.height() {
                return Err(unsupported("road_yapf", "non-flat reached road"));
            }
        }
        let bits = match kind {
            TileKind::Road => {
                if map.m5() & 0x30 != 0 || (map.m6() >> 3) & 7 >= 6 {
                    return Err(unsupported("road_yapf", "one-way or roadworks"));
                }
                [0_u16, 0, 0, 16, 0, 2, 8, 26, 0, 4, 1, 21, 32, 38, 41, 63]
                    .get(usize::from(map.m5() & 15))
                    .copied()
                    .ok_or_else(|| unsupported("road_yapf", "road bits"))?
            }
            TileKind::Depot(direction) | TileKind::Terminal { direction, .. } => {
                1_u16 << (direction & 1)
            }
            TileKind::Empty => 0,
        };
        Ok(Tile {
            kind,
            tracks: bits.wrapping_mul(0x101),
            owner: map.m1() & 31,
        })
    }
    pub(super) fn follow(&self, old: Position) -> Result<Option<(u32, u16)>, WorldTickError> {
        let exit = old.exit()?;
        let source = self.tile(old.tile)?;
        let reverse = match source.kind {
            TileKind::Depot(direction) => direction != exit,
            TileKind::Terminal { direction, .. } => direction == (exit ^ 2),
            TileKind::Empty | TileKind::Road => false,
        };
        if reverse {
            return Ok(Some((old.tile, 1_u16 << (old.track ^ 8))));
        }
        if matches!(source.kind, TileKind::Depot(direction) | TileKind::Terminal { direction, .. } if direction != exit)
        {
            return Ok(None);
        }
        let next = self.next(old.tile, exit)?;
        let target = self.tile(next)?;
        let tracks = target.tracks & reaches(exit)?;
        let allowed = match target.kind {
            TileKind::Depot(direction) => direction ^ 2 == exit && target.owner == self.owner,
            TileKind::Terminal { direction, .. } => direction ^ 2 == exit,
            TileKind::Empty | TileKind::Road => true,
        };
        if tracks != 0 && allowed {
            return Ok(Some((next, tracks)));
        }
        let tracks = source.tracks & reaches(exit ^ 2)?;
        Ok((tracks != 0).then_some((old.tile, tracks)))
    }
}
