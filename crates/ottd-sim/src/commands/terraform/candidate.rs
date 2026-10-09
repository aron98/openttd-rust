use super::super::terrain_read::TerrainRead;
use super::{CommandError, Failure, native};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Candidate<'a> {
    pub(super) world: TerrainRead<'a>,
    pub(super) heights: BTreeMap<u32, u8>,
    pub(super) dirty: BTreeSet<u32>,
    pub(super) freeform: bool,
    pub(super) maximum: u64,
    pub(super) price: i64,
}
enum Step {
    Enter {
        tile: u32,
        height: i16,
    },
    Neighbor {
        tile: u32,
        height: i16,
        direction: u8,
    },
}
impl Candidate<'_> {
    pub(super) fn height(&self, tile: u32) -> Result<u8, CommandError> {
        self.heights
            .get(&tile)
            .copied()
            .map_or_else(|| self.world.tile(tile).map(|tile| tile.height()), Ok)
    }
    pub(super) fn change(&mut self, tile: u32, height: i16) -> Result<i64, Failure> {
        let mut stack = vec![Step::Enter { tile, height }];
        let mut cost = 0_i64;
        let width = self.world.size().width();
        while let Some(step) = stack.pop() {
            match step {
                Step::Enter { tile, height } => {
                    if height < 0 {
                        return Err(native("STR_ERROR_ALREADY_AT_SEA_LEVEL", u32::MAX));
                    }
                    if u64::try_from(height).map_err(|_| CommandError::Overflow("height"))?
                        > self.maximum
                    {
                        return Err(native("STR_ERROR_TOO_HIGH", u32::MAX));
                    }
                    if height == i16::from(self.height(tile)?) {
                        return Err(native("CMD_ERROR", u32::MAX));
                    }
                    let divisor = std::num::NonZeroU32::new(width)
                        .ok_or(CommandError::Overflow("map width"))?;
                    let (x, y) = (tile % divisor, tile / divisor);
                    if !self.freeform
                        && (x <= 1
                            || y <= 1
                            || x >= width.saturating_sub(2)
                            || y >= self.world.size().height().saturating_sub(2))
                    {
                        let error_x = if x == 1 { 0 } else { x };
                        let error_y = if y == 1 { 0 } else { y };
                        let error_tile = error_y
                            .checked_mul(width)
                            .and_then(|n| n.checked_add(error_x))
                            .ok_or(CommandError::Overflow("error tile"))?;
                        return Err(native("STR_ERROR_TOO_CLOSE_TO_EDGE_OF_MAP", error_tile));
                    }
                    if y >= 1 {
                        self.dirty.insert(tile.wrapping_sub(width));
                    }
                    if y >= 1 && x >= 1 {
                        self.dirty.insert(tile.wrapping_sub(width).wrapping_sub(1));
                    }
                    if x >= 1 {
                        self.dirty.insert(tile.wrapping_sub(1));
                    }
                    self.dirty.insert(tile);
                    self.heights.insert(
                        tile,
                        u8::try_from(height).map_err(|_| CommandError::Overflow("tile height"))?,
                    );
                    cost = cost.saturating_add(self.price);
                    stack.push(Step::Neighbor {
                        tile,
                        height,
                        direction: 0,
                    });
                }
                Step::Neighbor {
                    tile,
                    height,
                    direction,
                } => {
                    if direction >= 4 {
                        continue;
                    }
                    stack.push(Step::Neighbor {
                        tile,
                        height,
                        direction: direction.wrapping_add(1),
                    });
                    if let Some(next) = neighbor(self.world, tile, direction) {
                        let old = i16::from(self.height(next)?);
                        if height.abs_diff(old) > 1 {
                            stack.push(Step::Enter {
                                tile: next,
                                height: if height > old {
                                    height.saturating_sub(1)
                                } else {
                                    height.saturating_add(1)
                                },
                            });
                        }
                    }
                }
            }
        }
        Ok(cost)
    }
}
pub(super) fn neighbor(world: TerrainRead<'_>, tile: u32, direction: u8) -> Option<u32> {
    let width = std::num::NonZeroU32::new(world.size().width())?;
    let (x, y) = (tile % width, tile / width);
    match direction {
        0 if x > 0 => tile.checked_sub(1),
        1 if y.checked_add(1)? < world.size().height() => tile.checked_add(width.get()),
        2 if x.checked_add(1)? < width.get() => tile.checked_add(1),
        3 if y > 0 => tile.checked_sub(width.get()),
        _ => None,
    }
}
