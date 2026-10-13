// SPDX-License-Identifier: GPL-2.0-only
//! Vanilla `TileLoop_Town` construction and original/binomial town production.
use super::{
    State, WorldTickError,
    house_cargo::{self, CargoSource, TileArea, TownCargoProduction},
    tiles, unsupported,
};
use crate::{
    Tile,
    content::{HouseSpec, vanilla_town_cargoes},
    runtime::{
        RuntimeError,
        towns::{north_part, spec},
    },
};
use ottd_save::world::{World, WorldEdit};

pub(super) fn validate(world: &World) -> Result<(), WorldTickError> {
    let width = world.map().width();
    for (index, tile) in world.map().tiles().iter().enumerate() {
        if tile.tile_type() >> 4 != 3 {
            continue;
        }
        let id = tile.m8() & 0x0fff;
        let house = spec(id)?;
        if house.building_flags & 32 != 0 {
            return Err(unsupported("houses", "vanilla lift animation"));
        }
        if tile.m3() & 128 == 0 && (tile.m5() >> 3) & 3 == 3 {
            return Err(unsupported(
                "houses",
                "inconsistent construction completion",
            ));
        }
        let (north, (dx, dy)) = north_part(id)?;
        let index = i64::try_from(index).map_err(|_| unsupported("houses", "map index"))?;
        let origin = i64::from(dy)
            .checked_mul(i64::from(width))
            .and_then(|value| value.checked_add(i64::from(dx)))
            .and_then(|offset| index.checked_add(offset))
            .ok_or_else(|| unsupported("houses", "north offset overflow"))?;
        let origin = usize::try_from(origin).map_err(|_| unsupported("houses", "north tile"))?;
        let main = world
            .map()
            .tiles()
            .get(origin)
            .ok_or_else(|| unsupported("houses", "north tile"))?;
        if main.tile_type() >> 4 != 3
            || main.m8() & 0x0fff != north
            || main.m2() != tile.m2()
            || main.m3() & 128 != tile.m3() & 128
            || (main.m3() & 128 == 0 && main.m5() & 31 != tile.m5() & 31)
        {
            return Err(unsupported("houses", "inconsistent multi-tile footprint"));
        }
        if house.is_north() {
            let origin = u32::try_from(origin).map_err(|_| unsupported("houses", "north index"))?;
            let (house_width, house_height) = house.size();
            let width = std::num::NonZeroU32::new(width)
                .ok_or_else(|| unsupported("houses", "map width"))?;
            if (origin % width)
                .checked_add(house_width)
                .is_none_or(|right| right > width.get())
                || (origin / width)
                    .checked_add(house_height)
                    .is_none_or(|bottom| bottom > world.map().height())
            {
                return Err(unsupported("houses", "footprint crosses map edge"));
            }
            let mut expected = id;
            for offset in offsets(house, width.get()) {
                let target = origin
                    .checked_add(offset)
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| world.map().tiles().get(i))
                    .ok_or_else(|| unsupported("houses", "footprint edge"))?;
                if target.tile_type() >> 4 != 3
                    || target.m8() & 0x0fff != expected
                    || target.m2() != tile.m2()
                {
                    return Err(unsupported("houses", "missing footprint part"));
                }
                expected = expected.wrapping_add(1);
            }
        }
    }
    Ok(())
}

fn offsets(house: HouseSpec, width: u32) -> Vec<u32> {
    let flags = house.building_flags;
    let mut result = Vec::new();
    if house.is_north() {
        result.push(0);
    }
    if flags & 0x18 != 0 {
        result.push(width);
    }
    if flags & 0x14 != 0 {
        result.push(1);
    }
    if flags & 16 != 0 {
        result.push(width.wrapping_add(1));
    }
    result
}

fn advance_construction(
    state: &mut State<'_>,
    index: u32,
    house: HouseSpec,
) -> Result<(), WorldTickError> {
    for offset in offsets(house, state.width) {
        let index = index
            .checked_add(offset)
            .ok_or_else(|| unsupported("houses", "construction footprint"))?;
        let mut tile = tiles::tile(&state.view().tile(index)?);
        tile.m5 = (tile.m5 & !31) | (tile.m5.wrapping_add(1) & 31);
        if (tile.m5 >> 3) & 3 == 3 {
            tile.m3 |= 128;
        }
        if tile.m3 & 128 != 0 {
            let population = spec(tile.m8 & 0x0fff)?.population;
            let town = state
                .towns
                .as_mut()
                .ok_or_else(|| unsupported("houses", "missing live town cache"))?
                .town_mut(tile.m2)?;
            town.population = town.population.wrapping_add(u32::from(population));
            tile.m5 = 0;
        }
        state.apply(WorldEdit::Tile {
            index,
            value: tiles::saved(&tile),
        })?;
    }
    Ok(())
}

pub(super) fn visit(
    state: &mut State<'_>,
    index: u32,
    tick: u64,
    delivery: &mut impl crate::WorldTickPhases,
) -> Result<(), WorldTickError> {
    let tile = state.view().tile(index)?;
    let house = spec(tile.m8() & 0x0fff)?;
    if tile.m3() & 128 == 0 {
        return advance_construction(state, index, house);
    }
    let _rebuild_draw = house_cargo::random(state)?;
    let mode = state.number(b"PATS", 0, "economy.town_cargogen_mode")?;
    if mode == 1 && (tick >> 8) & 3 != u64::from(index & 3) {
        return Ok(());
    }
    if !matches!(mode, 0 | 1) {
        return Err(unsupported("house_cargo", "generation mode"));
    }
    for (effect, rate) in [(1, house.population), (2, house.mail_generation)] {
        for (cargo, cargo_spec) in vanilla_town_cargoes(effect).map_err(RuntimeError::from)? {
            let draw = house_cargo::random(state)?;
            let multiplier = u32::from(cargo_spec.town_production_multiplier);
            let amount = if mode == 0 {
                let low = draw & 255;
                if low >= u32::from(rate) {
                    continue;
                }
                (low.wrapping_mul(multiplier) / 256 / 8).wrapping_add(1)
            } else {
                let bits = u32::from(rate).div_ceil(8);
                let mask = if bits >= 32 {
                    u32::MAX
                } else {
                    1_u32.wrapping_shl(bits).wrapping_sub(1)
                };
                (draw & mask).count_ones().wrapping_mul(multiplier) / 256
            };
            house_cargo::generate(
                state,
                TownCargoProduction {
                    source: CargoSource::Town(tile.m2()),
                    area: TileArea {
                        tile: index,
                        width: 1,
                        height: 1,
                    },
                    cargo,
                    amount,
                },
                delivery,
            )?;
        }
    }
    Ok(())
}

pub(super) fn yearly(state: &mut State<'_>) -> Result<(), WorldTickError> {
    for index in 0..state.width.wrapping_mul(state.height) {
        let before = state.view().tile(index)?;
        if before.tile_type() >> 4 != 3 || before.m3() & 128 == 0 || before.m5() == 255 {
            continue;
        }
        let mut tile = tiles::tile(&before);
        tile.m5 = tile.m5.wrapping_add(1);
        state.apply(WorldEdit::Tile {
            index,
            value: tiles::saved(&tile),
        })?;
    }
    Ok(())
}

pub(super) fn road(state: &State<'_>, tile: &mut Tile, index: u32) -> Result<(), WorldTickError> {
    if tile.m5 >> 6 != 0 {
        return Ok(());
    }
    let town = state
        .towns
        .as_ref()
        .ok_or_else(|| unsupported("road_tile_loop", "town cache"))?
        .town(tile.m2)?;
    let centre = u32::try_from(state.number(b"CITY", u32::from(tile.m2), "xy")?)
        .map_err(|_| unsupported("road_tile_loop", "town centre"))?;
    let width = std::num::NonZeroU32::new(state.width)
        .ok_or_else(|| unsupported("road_tile_loop", "map width"))?;
    let dx = (index % width).abs_diff(centre % width);
    let dy = (index / width).abs_diff(centre / width);
    let distance = u64::from(dx)
        .checked_mul(u64::from(dx))
        .zip(u64::from(dy).checked_mul(u64::from(dy)))
        .and_then(|(x, y)| x.checked_add(y))
        .ok_or_else(|| unsupported("road_tile_loop", "town distance overflow"))?;
    let mut group = 0;
    for (index, radius) in town.zone_radii.iter().enumerate() {
        if distance < u64::from(*radius) {
            group = index;
        }
    }
    let (desired, previous) = match group {
        0 => (1, 1),
        1 | 2 => (2, 2),
        3 => (5, 5),
        4 => (3, 2),
        _ => return Err(unsupported("road_tile_loop", "town zone")),
    };
    let current = (tile.m6 >> 3) & 7;
    let next = if current == desired || current == previous {
        desired
    } else if current == 0 {
        previous
    } else {
        0
    };
    tile.m6 = (tile.m6 & !0x38) | (next << 3);
    Ok(())
}
