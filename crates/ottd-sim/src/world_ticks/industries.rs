// SPDX-License-Identifier: GPL-2.0-only
//! Vanilla temperate forest: `industry_cmd.cpp` and `table/build_industry.h` (OpenTTD 15.3).
mod saved;
use super::{
    State, WorldTickError,
    house_cargo::{TileArea, random},
    tiles, unsupported,
};
use crate::cargo::{CargoSource, Production};
use crate::world_access::{field, unsigned};
use ottd_save::{
    WireValue,
    world::{World, WorldEdit},
};
pub(super) use saved::{monthly_state, publish_month};

const FOREST: u64 = 3;
const WOOD: u8 = 7;
const DEFAULT_RATE: u16 = 13;
const MINIMAL_CARGO: u16 = 30;

pub(super) fn validate(world: &World) -> Result<(), WorldTickError> {
    let table = world
        .tables()
        .get(b"INDY")
        .ok_or_else(|| unsupported("industries", "pool"))?;
    if table.records().len() > 1 {
        return Err(unsupported(
            "industries",
            "multiple industry production/closure",
        ));
    }
    for id in table.records().keys() {
        if unsigned(world, b"INDY", *id, "type")? != FOREST {
            return Err(unsupported(
                "industries",
                "non-forest specification/effects",
            ));
        }
        for name in ["ctlflags", "neutral_station", "psa"] {
            if unsigned(world, b"INDY", *id, name)? != 0 {
                return Err(unsupported("industries", name));
            }
        }
        if !(4..=128).contains(&unsigned(world, b"INDY", *id, "prod_level")?) {
            return Err(unsupported("industries", "closure or production level"));
        }
        if !matches!(field(world, b"INDY", *id, "accepted")?, WireValue::Structs(rows) if rows.is_empty())
        {
            return Err(unsupported("industries", "forest accepted cargo slots"));
        }
    }
    for tile in world.map().tiles() {
        if tile.tile_type() >> 4 != 8 {
            continue;
        }
        if !table.records().contains_key(&u32::from(tile.m2())) {
            return Err(unsupported("industries", "tile industry reference"));
        }
        if tile.m1() & 0x60 != 0x60 {
            return Err(unsupported("industries", "industry water tile loop"));
        }
        if tile.m5() != 16 || tile.m6() & 4 != 0 {
            return Err(unsupported(
                "industries",
                "forest graphics animation/effects",
            ));
        }
        if tile.m1() & 128 == 0 && tile.m1() & 3 == 3 {
            return Err(unsupported(
                "industries",
                "inconsistent construction completion",
            ));
        }
    }
    Ok(())
}

pub(super) fn visit(
    state: &mut State<'_>,
    index: u32,
    phases: &mut impl crate::WorldTickPhases,
) -> Result<(), WorldTickError> {
    let mut tile = tiles::tile(&state.view().tile(index)?);
    if tile.m1 & 128 == 0 {
        let count = ((tile.m1 >> 2) & 3).wrapping_add(1);
        tile.m1 = (tile.m1 & !0x0c) | ((count & 3) << 2);
        if count == 4 {
            let progress = (tile.m1 & 3).wrapping_add(1);
            tile.m1 = (tile.m1 & !3) | progress;
            let _animation_draw = random(state)?;
            if progress == 3 {
                tile.m1 |= 128;
            }
        }
        state.apply(WorldEdit::Tile {
            index,
            value: tiles::saved(&tile),
        })?;
        return Ok(());
    }
    let id = u32::from(tile.m2);
    let mut output = saved::produced(state, id)?;
    let amount = output.waiting.min(u16::from(u8::MAX));
    if amount > MINIMAL_CARGO {
        output.waiting = output.waiting.wrapping_sub(amount);
        let history = output
            .history
            .first_mut()
            .ok_or_else(|| unsupported("industry_cargo", "history"))?;
        history.production = history.production.wrapping_add(amount);
        saved::set_produced(state, id, &output)?;
        let production = Production {
            source: CargoSource::Industry(tile.m2),
            area: saved::area(state, id)?,
            cargo: WOOD,
            amount: u32::from(amount),
        };
        let delivered = phases.industry_cargo(state, production)?;
        if delivered != 0 {
            return Err(unsupported(
                "industry_cargo",
                "delivered forest cargo animation",
            ));
        }
    }
    // Vanilla has no animation callback, but the wrapper draws before checking it.
    let _animation_draw = random(state)?;
    Ok(())
}

pub(super) fn tick(state: &mut State<'_>) -> Result<(), WorldTickError> {
    let Some((id, _)) = state.table(b"INDY")?.records().next() else {
        return Ok(());
    };
    let old = u16::try_from(state.number(b"INDY", id, "counter")?)
        .map_err(|_| unsupported("industries", "counter"))?;
    if old % 64 == 0 {
        let _sound_draw = random(state)?;
    }
    let counter = old.wrapping_sub(1);
    state.set_number(b"INDY", id, "counter", i64::from(counter))?;
    if counter % 256 == 0 {
        let mut output = saved::produced(state, id)?;
        output.waiting = output.waiting.saturating_add(scaled(state, output.rate)?);
        saved::set_produced(state, id, &output)?;
        let _animation_draw = random(state)?;
    }
    Ok(())
}

fn scaled(state: &mut State<'_>, amount: u8) -> Result<u16, WorldTickError> {
    if amount == 0 {
        return Ok(0);
    }
    let percentage = u16::try_from(state.number(b"PATS", 0, "economy.industry_cargo_scale")?)
        .map_err(|_| unsupported("industries", "cargo scale"))?;
    if !(15..=300).contains(&percentage) {
        return Err(unsupported("industries", "cargo scale setting"));
    }
    let scaled = u16::try_from(u32::from(amount).saturating_mul(u32::from(percentage)) / 100)
        .map_err(|_| unsupported("industries", "scaled production"))?;
    if scaled != 0 {
        return Ok(scaled);
    }
    let denominator = u32::from(
        100_u16
            .checked_div(percentage)
            .ok_or_else(|| unsupported("industries", "zero scale"))?
            .max(1),
    );
    Ok(u16::from(chance(state, denominator)?))
}

fn chance(state: &mut State<'_>, denominator: u32) -> Result<bool, WorldTickError> {
    Ok(((random(state)? & 0xffff)
        .wrapping_mul(denominator)
        .wrapping_add(denominator / 2)
        >> 16)
        < 1)
}

pub(super) fn change_production(state: &mut State<'_>, id: u32) -> Result<(), WorldTickError> {
    if !chance(state, 3)? {
        return Ok(());
    }
    let mut output = saved::produced(state, id)?;
    let last = output
        .history
        .get(1)
        .ok_or_else(|| unsupported("industry_day", "last month"))?;
    let transported = u32::from(last.transported)
        .saturating_mul(256)
        .checked_div(u32::from(last.production))
        .unwrap_or(0)
        .min(255);
    let increase = (transported > 153) != chance(state, 3)?;
    let old = u16::try_from(state.number(b"INDY", id, "prod_level")?)
        .map_err(|_| unsupported("industry_day", "production level"))?;
    let next = if increase {
        old.saturating_mul(2).min(128)
    } else {
        (old / 2).max(4)
    };
    // The last vanilla forest is protected from closure; the level remains four.
    if old != next {
        state.set_number(b"INDY", id, "prod_level", i64::from(next))?;
        output.rate = u8::try_from(DEFAULT_RATE.saturating_mul(next).div_ceil(16))
            .map_err(|_| unsupported("industry_day", "rate"))?;
        saved::set_produced(state, id, &output)?;
    }
    Ok(())
}

pub(crate) fn deliver_empty(
    state: &State<'_>,
    production: Production,
) -> Result<u32, WorldTickError> {
    if state.table(b"STNN")?.records().next().is_some() {
        return Err(unsupported(
            "industry_cargo",
            "station distribution and packet allocation",
        ));
    }
    let CargoSource::Industry(source) = production.source else {
        return Err(unsupported("industry_cargo", "industry producer source"));
    };
    let area = saved::area(state, u32::from(source))?;
    if production.cargo != WOOD
        || production.amount == 0
        || production.area.tile != area.tile
        || production.area.width != area.width
        || production.area.height != area.height
    {
        return Err(unsupported("industry_cargo", "producer source or area"));
    }
    Ok(0)
}
