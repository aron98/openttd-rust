//! Geometry queries over the authoritative saved map.
use ottd_core::terrain::{GeometryError, Slope, slope_from_corners};
use ottd_save::world::World;

/// Native `GetTileSlopeZ`, including clamped corners on southern/eastern map borders.
/// # Errors
/// Rejects an out-of-map index or corner heights that cannot form a native terrain slope.
pub fn tile_slope_z(world: &World, index: u32) -> Result<(Slope, u8), GeometryError> {
    let map = world.map();
    let width = std::num::NonZeroU32::new(map.width()).ok_or(GeometryError::Tile)?;
    let x = index % width;
    let y = index / width;
    if y >= map.height() {
        return Err(GeometryError::Tile);
    }
    let x2 = x.saturating_add(1).min(map.width().saturating_sub(1));
    let y2 = y.saturating_add(1).min(map.height().saturating_sub(1));
    let height = |x: u32, y: u32| {
        let index = y
            .checked_mul(map.width())
            .and_then(|n| n.checked_add(x))
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(GeometryError::Tile)?;
        map.tiles()
            .get(index)
            .map(ottd_save::TileState::height)
            .ok_or(GeometryError::Tile)
    };
    slope_from_corners([
        height(x, y)?,
        height(x2, y)?,
        height(x, y2)?,
        height(x2, y2)?,
    ])
}
