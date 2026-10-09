use super::{CommandCost, CommandError, CommandReturn, Plan, landscape};
use crate::{
    content::{ContentCatalog, Price},
    terrain::tile_slope_z,
    world_access::{field_edit, unsigned},
};
use ottd_save::{
    TileRawParts, WireValue,
    world::{World, WorldEdit},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
enum Failure {
    Scope(CommandError),
    Native { cost: CommandCost, tile: u32 },
}
impl From<CommandError> for Failure {
    fn from(error: CommandError) -> Self {
        Self::Scope(error)
    }
}
fn native(symbol: &str, tile: u32) -> Failure {
    Failure::Native {
        cost: CommandCost::failure(symbol),
        tile,
    }
}
const fn outcome(cost: CommandCost, tile: u32, edits: Vec<WorldEdit>) -> Plan {
    Plan {
        cost,
        edits,
        returns: Some(CommandReturn::Landscape {
            additional_money: 0,
            tile,
        }),
    }
}
pub(super) fn plan(
    world: &World,
    company: u8,
    tile: u32,
    mask: u8,
    up: bool,
) -> Result<Plan, CommandError> {
    match build(world, company, tile, mask, up) {
        Ok(plan) => Ok(plan),
        Err(Failure::Scope(error)) => Err(error),
        Err(Failure::Native { cost, tile }) => Ok(outcome(cost, tile, Vec::new())),
    }
}

struct Candidate<'a> {
    world: &'a World,
    heights: BTreeMap<u32, u8>,
    dirty: BTreeSet<u32>,
    freeform: bool,
    maximum: u64,
    price: i64,
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
    fn height(&self, tile: u32) -> Result<u8, CommandError> {
        self.heights.get(&tile).copied().map_or_else(
            || landscape::tile_at(self.world, tile).map(ottd_save::TileState::height),
            Ok,
        )
    }
    fn change(&mut self, tile: u32, height: i16) -> Result<i64, Failure> {
        let mut stack = vec![Step::Enter { tile, height }];
        let mut cost = 0_i64;
        let width = self.world.map().width();
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
                            || y >= self.world.map().height().saturating_sub(2))
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
fn neighbor(world: &World, tile: u32, direction: u8) -> Option<u32> {
    let width = std::num::NonZeroU32::new(world.map().width())?;
    let (x, y) = (tile % width, tile / width);
    match direction {
        0 if x > 0 => tile.checked_sub(1),
        1 if y.checked_add(1)? < world.map().height() => tile.checked_add(width.get()),
        2 if x.checked_add(1)? < width.get() => tile.checked_add(1),
        3 if y > 0 => tile.checked_sub(width.get()),
        _ => None,
    }
}
fn tunnel_in_way(world: &World, tile: u32, z: u8) -> Result<bool, CommandError> {
    let width = std::num::NonZeroU32::new(world.map().width())
        .ok_or(CommandError::Overflow("map width"))?;
    let (x, y) = (tile % width, tile / width);
    let directions = [
        if x > width.get().saturating_sub(1) / 2 {
            0_u8
        } else {
            2
        },
        if y > world.map().height().saturating_sub(1) / 2 {
            3
        } else {
            1
        },
    ];
    for direction in directions {
        let mut cursor = tile;
        while let Some(next) = neighbor(world, cursor, direction ^ 2) {
            cursor = next;
            let source = landscape::tile_at(world, cursor)?;
            if source.tile_type() >> 4 == 7 {
                break;
            }
            let (_, height) = tile_slope_z(world, cursor)
                .map_err(|_| CommandError::Unsupported("invalid tunnel scan geometry"))?;
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
fn build(world: &World, company: u8, tile: u32, mask: u8, up: bool) -> Result<Plan, Failure> {
    let catalog = ContentCatalog::from_world(world).map_err(CommandError::from)?;
    let width = world.map().width();
    let count =
        u32::try_from(world.map().tiles().len()).map_err(|_| CommandError::Overflow("map size"))?;
    let mut state = Candidate {
        world,
        heights: BTreeMap::new(),
        dirty: BTreeSet::new(),
        freeform: unsigned(world, b"PATS", 0, "construction.freeform_edges")
            .map_err(CommandError::from)?
            != 0,
        maximum: unsigned(world, b"PATS", 0, "construction.map_height_limit")
            .map_err(CommandError::from)?,
        price: catalog.prices().get(Price::Terraform),
    };
    let mut cost = 0_i64;
    for (bit, offset) in [(1, 1), (2, width.saturating_add(1)), (4, width), (8, 0)] {
        if mask & bit == 0 {
            continue;
        }
        let Some(corner) = tile.checked_add(offset).filter(|n| *n < count) else {
            continue;
        };
        let height = i16::from(landscape::tile_at(world, corner)?.height());
        cost = cost.saturating_add(state.change(
            corner,
            if up {
                height.saturating_add(1)
            } else {
                height.saturating_sub(1)
            },
        )?);
    }
    let (surface_cost, mut tiles) = clear_surfaces(&state, company, up, catalog.prices())?;
    cost = cost.saturating_add(surface_cost);
    let limit = unsigned(world, b"PLYR", u32::from(company), "terraform_limit")
        .map_err(CommandError::from)?;
    let changed = u64::try_from(state.heights.len())
        .map_err(|_| CommandError::Overflow("changed corners"))?;
    if ((limit >> 16) & 65535) < changed {
        return Err(native("STR_ERROR_TERRAFORM_LIMIT_REACHED", u32::MAX));
    }
    for (index, height) in state.heights {
        let parts = tiles
            .entry(index)
            .or_insert(TileRawParts::from(landscape::tile_at(world, index)?));
        parts.height = height;
    }
    let mut edits: Vec<_> = tiles
        .into_iter()
        .map(|(index, parts)| WorldEdit::Tile {
            index,
            value: parts.into(),
        })
        .collect();
    edits.push(field_edit(
        *b"PLYR",
        u32::from(company),
        "terraform_limit",
        WireValue::Unsigned(limit.wrapping_sub(changed << 16)),
    ));
    Ok(outcome(CommandCost::success(cost, 0), tile, edits))
}

fn clear_surfaces(
    state: &Candidate<'_>,
    company: u8,
    up: bool,
    prices: &crate::content::Prices,
) -> Result<(i64, BTreeMap<u32, TileRawParts>), Failure> {
    let world = state.world;
    let width = world.map().width();
    let mut cost = 0_i64;
    let mut tiles = BTreeMap::<u32, TileRawParts>::new();
    for pass in 0..2 {
        for &dirty in &state.dirty {
            let source = landscape::tile_at(world, dirty)?;
            if source.tile_type() >> 4 == 7 {
                continue;
            }
            let corners = [
                state.height(dirty)?,
                state.height(dirty.wrapping_add(1))?,
                state.height(dirty.wrapping_add(width))?,
                state.height(dirty.wrapping_add(width).wrapping_add(1))?,
            ];
            let (_, base) = ottd_core::terrain::slope_from_corners(corners)
                .map_err(|_| CommandError::Unsupported("invalid candidate terrain geometry"))?;
            if pass == 0 {
                if source.tile_type() & 0x0C != 0 {
                    return Err(CommandError::Unsupported("terraform beneath bridge").into());
                }
                if !up && tunnel_in_way(world, dirty, base)? {
                    return Err(native("STR_ERROR_EXCAVATION_WOULD_DAMAGE", dirty));
                }
            }
            let clear = landscape::clear_with_prices(world, company, dirty, true, prices)?;
            if !clear.cost.success {
                return Err(Failure::Native {
                    cost: clear.cost,
                    tile: dirty,
                });
            }
            if pass == 1 {
                cost = cost.saturating_add(clear.cost.cost);
                for edit in clear.edits {
                    match edit {
                        WorldEdit::Tile { index, value } => {
                            tiles.insert(index, TileRawParts::from(&value));
                        }
                        _ => {
                            return Err(CommandError::Unsupported(
                                "unexpected automatic clear object edit",
                            )
                            .into());
                        }
                    }
                }
            }
        }
    }
    Ok((cost, tiles))
}
