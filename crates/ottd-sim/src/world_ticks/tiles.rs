use crate::{Map, Tile};
use ottd_save::world::World;

pub(super) const fn tile(t: &ottd_save::TileState) -> Tile {
    Tile {
        tile_type: t.tile_type(),
        height: t.height(),
        m1: t.m1(),
        m2: t.m2(),
        m3: t.m3(),
        m4: t.m4(),
        m5: t.m5(),
        m6: t.m6(),
        m7: t.m7(),
        m8: t.m8(),
    }
}

pub(super) fn map(world: &World) -> Map {
    Map {
        width: world.map().width(),
        height: world.map().height(),
        tiles: world
            .map()
            .tiles()
            .iter()
            .map(|t| Tile {
                tile_type: t.tile_type(),
                height: t.height(),
                m1: t.m1(),
                m2: t.m2(),
                m3: t.m3(),
                m4: t.m4(),
                m5: t.m5(),
                m6: t.m6(),
                m7: t.m7(),
                m8: t.m8(),
            })
            .collect(),
    }
}

pub(super) fn saved(tile: &Tile) -> ottd_save::TileState {
    ottd_save::TileState::from(ottd_save::TileRawParts {
        tile_type: tile.tile_type,
        height: tile.height,
        m1: tile.m1,
        m2: tile.m2,
        m3: tile.m3,
        m4: tile.m4,
        m5: tile.m5,
        m6: tile.m6,
        m7: tile.m7,
        m8: tile.m8,
    })
}
