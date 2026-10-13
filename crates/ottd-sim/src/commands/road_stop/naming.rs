use crate::{
    commands::{CommandError, landscape},
    runtime::stations::saved,
    world_access::unsigned,
};
use ottd_save::world::World;
use std::collections::BTreeSet;

pub(super) fn name(world: &World, tile: u32, town: u32) -> Result<u16, CommandError> {
    let mut used = used_names(world, town)?;
    let width = std::num::NonZeroU32::new(world.map().width())
        .ok_or(CommandError::Overflow("map width"))?;
    let (x, y) = (tile % width, tile / width);
    let centre = u32::try_from(unsigned(world, b"CITY", town, "xy")?)
        .map_err(|_| CommandError::Overflow("town tile"))?;
    let (tx, ty) = (centre % width, centre / width);
    let (mut water, mut trees) = (0_u32, 0_u32);
    for dy in -3..=3 {
        for dx in -3..=3 {
            let Some(px) = x
                .checked_add_signed(dx)
                .filter(|x| *x < world.map().width())
            else {
                continue;
            };
            let Some(py) = y
                .checked_add_signed(dy)
                .filter(|y| *y < world.map().height())
            else {
                continue;
            };
            let index = py
                .checked_mul(width.get())
                .and_then(|n| n.checked_add(px))
                .ok_or(CommandError::Overflow("name tile"))?;
            let source = landscape::tile_at(world, index)?;
            match source.tile_type() >> 4 {
                8 => return Err(CommandError::Unsupported("industry station naming")),
                6 if source.m5() >> 4 == 0 => water = water.saturating_add(1),
                4 => trees = trees.saturating_add(1),
                _ => {}
            }
        }
    }
    if x.abs_diff(tx).max(y.abs_diff(ty)) < 8 {
        for name in [0x6006, 0x600b] {
            if !used.contains(&name) {
                return Ok(name);
            }
        }
    }
    let edge = x
        .min(y)
        .min(world.map().width().saturating_sub(1).saturating_sub(x))
        .min(world.map().height().saturating_sub(1).saturating_sub(y));
    if edge < 20 && water >= 5 && !used.contains(&0x6011) {
        return Ok(0x6011);
    }
    if trees >= 8 && !used.contains(&0x6010) {
        return Ok(
            if unsigned(world, b"PATS", 0, "game_creation.landscape")? == 2 {
                0x6026
            } else {
                0x6010
            },
        );
    }
    let z = landscape::tile_at(world, tile)?.height();
    let town_z = landscape::tile_at(world, centre)?.height();
    if z < town_z && !used.contains(&0x600e) {
        return Ok(0x600e);
    }
    if z > town_z && !used.contains(&0x600f) {
        return Ok(0x600f);
    }
    used.extend(if x < tx {
        [0x6008, 0x600a]
    } else {
        [0x6007, 0x6009]
    });
    used.extend(if y < ty {
        [0x6008, 0x6009]
    } else {
        [0x6007, 0x600a]
    });
    Ok([
        0x6007, 0x6008, 0x6009, 0x600a, 0x600c, 0x600d, 0x6012, 0x6020, 0x6021, 0x6022, 0x6023,
        0x6024,
    ]
    .into_iter()
    .find(|n| !used.contains(n))
    .unwrap_or(0x6027))
}

fn used_names(world: &World, town: u32) -> Result<BTreeSet<u16>, CommandError> {
    let mut used = BTreeSet::new();
    let stations = world
        .tables()
        .get(b"STNN")
        .ok_or(CommandError::Unsupported("station table"))?;
    for id in stations.records().keys() {
        let row = saved::row(world, *b"STNN", *id)?;
        if row.number("facilities")? & 128 != 0 {
            continue;
        }
        let normal = row.child("normal")?;
        let base = normal.child("base")?;
        if base.number("town")? != u64::from(town) + 1 || normal.number("indtype")? != 255 {
            continue;
        }
        let name = u16::try_from(base.number("string_id")?)
            .map_err(|_| CommandError::Overflow("station name"))?;
        if (0x6006..0x6027).contains(&name) {
            used.insert(if name == 0x6026 { 0x6010 } else { name });
        }
    }
    Ok(used)
}
