use super::super::terrain_read::TerrainRead;
use super::{CommandError, candidate::neighbor};

pub(super) fn tunnel_in_way(
    world: TerrainRead<'_>,
    tile: u32,
    z: u8,
) -> Result<bool, CommandError> {
    let width = std::num::NonZeroU32::new(world.size().width())
        .ok_or(CommandError::Overflow("map width"))?;
    let (x, y) = (tile % width, tile / width);
    let directions = [
        if x > width.get().saturating_sub(1) / 2 {
            0_u8
        } else {
            2
        },
        if y > world.size().height().saturating_sub(1) / 2 {
            3
        } else {
            1
        },
    ];
    for direction in directions {
        let mut cursor = tile;
        while let Some(next) = neighbor(world, cursor, direction ^ 2) {
            cursor = next;
            let source = world.tile(cursor)?;
            if source.tile_type() >> 4 == 7 {
                break;
            }
            let height = world.base_height(cursor)?;
            if z < height {
                continue;
            }
            if z == height
                && source.tile_type() >> 4 == 9
                && source.m5() & 128 == 0
                && source.m5() & 3 == direction
            {
                return Ok(true);
            }
            break;
        }
    }
    Ok(false)
}
