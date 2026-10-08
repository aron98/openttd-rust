use crate::Map;
use ottd_core::MapDimensions;

const FEEDBACKS: [usize; 13] = [
    0xD8F,
    0x1296,
    0x2496,
    0x4357,
    0x8679,
    0x1030E,
    0x206CD,
    0x403FE,
    0x807B8,
    0x0010_04B2,
    0x0020_06A8,
    0x0040_04B2,
    0x0080_0B87,
];

/// Input outside the supported temperate clear landscape contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LandscapeError {
    /// Invalid map dimensions or tile count.
    #[error("invalid map dimensions or tile count")]
    Map,
    /// Tile requires an unported tile procedure or climate rule.
    #[error("unsupported terrain at tile {0}; expected snow-free grass, rough, rocks or void")]
    Tile(usize),
    /// A void neighbor may flood; water procedures are outside this domain.
    #[error(
        "potential flooding beside void tile {0}; neighboring clear tiles require all corner heights above zero"
    )]
    UnsupportedBoundary(usize),
    /// The tile scheduler requires an in-bounds nonzero cursor.
    #[error("tile-loop cursor must be nonzero and inside the map")]
    Cursor,
}

/// Validated, mutable normal-game temperate terrain without `NewGRF` callbacks.
#[derive(Debug, Clone)]
pub struct Landscape {
    map: Map,
    cursor: usize,
    feedback: usize,
    visits: usize,
}

impl Landscape {
    /// Takes ownership of a raw map and validates every tile before simulation.
    /// # Errors
    /// Rejects invalid dimensions, cursors, snowy tiles or unsupported tile kinds.
    pub fn new(map: Map, cursor: u32) -> Result<Self, LandscapeError> {
        let dimensions =
            MapDimensions::new(map.width, map.height).map_err(|_| LandscapeError::Map)?;
        let count = usize::try_from(dimensions.tile_count()).map_err(|_| LandscapeError::Map)?;
        if map.tiles.len() != count {
            return Err(LandscapeError::Map);
        }
        let cursor = usize::try_from(cursor).map_err(|_| LandscapeError::Cursor)?;
        if cursor == 0 || cursor >= count {
            return Err(LandscapeError::Cursor);
        }
        for (index, tile) in map.tiles.iter().enumerate() {
            match tile.tile_type >> 4 {
                0 if tile.m3 & 16 == 0 && (tile.m5 >> 2) & 7 <= 2 => {}
                7 => {}
                _ => return Err(LandscapeError::Tile(index)),
            }
        }
        validate_boundary(&map)?;
        let bits = count.trailing_zeros().saturating_sub(12);
        let feedback = *FEEDBACKS
            .get(usize::try_from(bits).map_err(|_| LandscapeError::Map)?)
            .ok_or(LandscapeError::Map)?;
        Ok(Self {
            map,
            cursor,
            feedback,
            visits: count >> 8,
        })
    }

    /// Runs the upstream tile scheduler using the already advanced game tick.
    pub fn advance(&mut self, tick_counter: u64) {
        let mut count = self.visits;
        if tick_counter % 256 == 0 {
            self.visit(0);
            count = count.saturating_sub(1);
        }
        for _ in 0..count {
            self.visit(self.cursor);
            self.cursor = (self.cursor >> 1)
                ^ if self.cursor & 1 == 1 {
                    self.feedback
                } else {
                    0
                };
        }
    }

    fn visit(&mut self, index: usize) {
        #[expect(
            clippy::indexing_slicing,
            reason = "validated nonzero maximal LFSR stays inside its map"
        )]
        let tile = &mut self.map.tiles[index];
        if tile.tile_type >> 4 != 0 || (tile.m5 >> 2) & 7 != 0 || tile.m5 & 3 == 3 {
            return;
        }
        tile.m5 = if tile.m5 >> 5 < 7 {
            tile.m5.wrapping_add(32)
        } else {
            (tile.m5 & 31).wrapping_add(1)
        };
    }

    /// Current map, including all unchanged raw tile bits.
    pub const fn map(&self) -> &Map {
        &self.map
    }

    /// Returns the owned map after simulation without copying tile storage.
    pub fn into_map(self) -> Map {
        self.map
    }

    /// Next nonzero tile scheduled by the LFSR.
    pub const fn cursor(&self) -> usize {
        self.cursor
    }
}

fn validate_boundary(map: &Map) -> Result<(), LandscapeError> {
    let width = usize::try_from(map.width).map_err(|_| LandscapeError::Map)?;
    let height = usize::try_from(map.height).map_err(|_| LandscapeError::Map)?;
    for (index, tile) in map.tiles.iter().enumerate() {
        if tile.tile_type >> 4 != 7 {
            continue;
        }
        let (x, y) = (
            index & width.wrapping_sub(1),
            index >> width.trailing_zeros(),
        );
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    continue;
                };
                if nx >= width || ny >= height {
                    continue;
                }
                let neighbor = ny.wrapping_mul(width).wrapping_add(nx);
                let candidate = map.tiles.get(neighbor).ok_or(LandscapeError::Map)?;
                if candidate.tile_type >> 4 == 7 {
                    continue;
                }
                let east = nx.saturating_add(1).min(width.saturating_sub(1));
                let south = ny.saturating_add(1).min(height.saturating_sub(1));
                for (cx, cy) in [(nx, ny), (east, ny), (nx, south), (east, south)] {
                    let corner = map
                        .tiles
                        .get(cy.wrapping_mul(width).wrapping_add(cx))
                        .ok_or(LandscapeError::Map)?;
                    if corner.height == 0 {
                        return Err(LandscapeError::UnsupportedBoundary(index));
                    }
                }
            }
        }
    }
    Ok(())
}
