//! Geometry queries over the authoritative saved map.
use ottd_core::terrain::{GeometryError, Slope, slope_from_corners};
use ottd_save::{TileState, world::World};

/// Native `GetTileSlopeZ`, including clamped corners on southern/eastern map borders.
/// # Errors
/// Rejects an out-of-map index or corner heights that cannot form a native terrain slope.
pub fn tile_slope_z(world: &World, index: u32) -> Result<(Slope, u8), GeometryError> {
    let map = world.map();
    slope_from_corners(corners(map.tiles(), map.width(), index)?)
}

pub(crate) fn tile_minimum_height(
    tiles: &[TileState],
    width: u32,
    index: u32,
) -> Result<u8, GeometryError> {
    let [north, west, east, south] = corners(tiles, width, index)?;
    Ok(north.min(west).min(east).min(south))
}

fn corners(tiles: &[TileState], width: u32, index: u32) -> Result<[u8; 4], GeometryError> {
    let width = std::num::NonZeroU32::new(width).ok_or(GeometryError::Tile)?;
    let count = u32::try_from(tiles.len()).map_err(|_| GeometryError::Tile)?;
    if count == 0 || count % width != 0 {
        return Err(GeometryError::Tile);
    }
    let map_height = count / width;
    let x = index % width;
    let y = index / width;
    if y >= map_height {
        return Err(GeometryError::Tile);
    }
    let x2 = x.saturating_add(1).min(width.get().saturating_sub(1));
    let y2 = y.saturating_add(1).min(map_height.saturating_sub(1));
    let height = |x: u32, y: u32| {
        let index = y
            .checked_mul(width.get())
            .and_then(|n| n.checked_add(x))
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(GeometryError::Tile)?;
        tiles
            .get(index)
            .map(ottd_save::TileState::height)
            .ok_or(GeometryError::Tile)
    };
    Ok([
        height(x, y)?,
        height(x2, y)?,
        height(x, y2)?,
        height(x2, y2)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ottd_save::TileRawParts;

    #[test]
    fn minimum_height_clamps_rectangular_edges_without_slope_validation()
    -> Result<(), GeometryError> {
        let tiles: Vec<TileState> = [7, 6, 5, 4, 3, 2]
            .into_iter()
            .map(|height| {
                TileRawParts {
                    tile_type: 0,
                    height,
                    m1: 0,
                    m2: 0,
                    m3: 0,
                    m4: 0,
                    m5: 0,
                    m6: 0,
                    m7: 0,
                    m8: 0,
                }
                .into()
            })
            .collect();
        assert_eq!(tile_minimum_height(&tiles, 3, 0)?, 3);
        assert_eq!(tile_minimum_height(&tiles, 3, 2)?, 2);
        assert_eq!(tile_minimum_height(&tiles, 3, 3)?, 3);
        assert_eq!(tile_minimum_height(&tiles, 3, 5)?, 2);
        assert!(tile_minimum_height(&tiles, 3, 6).is_err());
        assert!(tile_minimum_height(&tiles, 0, 0).is_err());
        assert!(tile_minimum_height(&tiles, 4, 0).is_err());
        Ok(())
    }
}
