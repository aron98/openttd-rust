use super::{LandscapeError, Map};
use crate::Tile;

pub(super) fn validate(map: &Map, index: usize) -> Result<(), LandscapeError> {
    let tile = map.tiles.get(index).ok_or(LandscapeError::Tile(index))?;
    let kind = tile.m5 >> 6;
    let supported = match kind {
        0 => tile.m5 & 15 != 0 && (tile.m6 >> 3) & 7 <= 1,
        2 => true,
        _ => false,
    };
    if !supported
        || tile.tile_type & 0x0c != 0
        || tile.m4 & 63 != 0
        || (tile.m8 >> 6) & 63 != 63
        || tile.m3 & 15 != 0
        || tile.m7 & 32 != 0
    {
        return Err(LandscapeError::Tile(index));
    }
    let width = usize::try_from(map.width).map_err(|_| LandscapeError::Map)?;
    if index.checked_rem(width).ok_or(LandscapeError::Map)? == width.saturating_sub(1) {
        return Err(LandscapeError::Tile(index));
    }
    for offset in [1, width, width.saturating_add(1)] {
        let corner = index
            .checked_add(offset)
            .and_then(|next| map.tiles.get(next))
            .ok_or(LandscapeError::Tile(index))?;
        if corner.height != tile.height {
            return Err(LandscapeError::Tile(index));
        }
    }
    Ok(())
}

pub(super) const fn visit(tile: &mut Tile) {
    if tile.m5 >> 6 == 0 {
        // House-free, unfunded towns always select Grass; only Barren/Grass is admitted.
        tile.m6 = (tile.m6 & !0x38) | 8;
    }
}
