use super::{WorldTickError, unsupported};
use crate::world_access::{field, signed, unsigned};
use ottd_save::{WireValue, world::World};

/// Returns whether this saved-world context admits empty-road callbacks.
pub(super) fn validate(world: &World, paused: bool) -> Result<bool, WorldTickError> {
    if unsigned(world, b"DATE", 0, "pause_mode")? & 64 != 0 {
        return Err(unsupported("pause", "linkgraph pause control"));
    }
    if !matches!(field(world,b"GSDT",0,"name")?,WireValue::Bytes(bytes) if bytes.is_empty()) {
        return Err(unsupported("scripts", "active GameScript"));
    }
    let gs = world
        .tables()
        .get(b"GSDT")
        .and_then(|t| t.records().get(&0))
        .ok_or_else(|| unsupported("scripts", "missing GameScript"))?;
    if gs.tail() != [0] {
        return Err(unsupported("scripts", "GameScript state"));
    }
    if paused {
        return Ok(false);
    }
    for id in [
        *b"NGRF", *b"VEHS", *b"INDY", *b"STNN", *b"OBJS", *b"LGRP", *b"LGRJ", *b"SUBS", *b"CAPA",
        *b"CAPY",
    ] {
        if world
            .tables()
            .get(&id)
            .is_none_or(|t| !t.records().is_empty())
        {
            return Err(unsupported(
                "domain",
                &format!("nonempty {}", String::from_utf8_lossy(&id)),
            ));
        }
    }
    for (id, name) in [
        (b"ANIT", "tiles"),
        (b"LGRS", "running"),
        (b"LGRS", "schedule"),
    ] {
        if !matches!(field(world,id,0,name)?,WireValue::Array(items) if items.is_empty()) {
            return Err(unsupported("domain", name));
        }
    }
    for name in [
        "game_creation.landscape",
        "economy.inflation",
        "economy.infrastructure_maintenance",
        "difficulty.economy",
        "difficulty.subsidy_duration",
        "difficulty.disasters",
        "difficulty.max_no_competitors",
        "economy.type",
        "economy.town_growth_rate",
    ] {
        if unsigned(world, b"PATS", 0, name)? != 0 {
            return Err(unsupported("settings", name));
        }
    }
    if !matches!(
        unsigned(world, b"PATS", 0, "construction.extra_tree_placement")?,
        0 | 3
    ) {
        return Err(unsupported("trees", "tree spreading"));
    }
    if signed(world, b"ECMY", 0, "fluct")? <= 0 {
        return Err(unsupported("economy", "recession"));
    }
    let date = i32::try_from(signed(world, b"DATE", 0, "date")?)
        .map_err(|_| unsupported("calendar", "date range"))?;
    let calendar = ottd_core::CalendarDate::from_raw(date)
        .map_err(|_| unsupported("calendar", "date range"))?;
    if calendar.ymd().0 < 2100 {
        return Err(unsupported(
            "engines",
            "calendar before vanilla engine aging stops",
        ));
    }
    validate_towns(world)?;
    validate_companies(world)?;
    validate_roads(world)
}
fn validate_companies(world: &World) -> Result<(), WorldTickError> {
    let companies = world
        .tables()
        .get(b"PLYR")
        .ok_or_else(|| unsupported("company", "pool"))?;
    for id in companies.records().keys() {
        for name in ["is_ai", "bankrupt_asked", "months_of_bankruptcy", "name_1"] {
            if unsigned(world, b"PLYR", *id, name)? != 0 {
                return Err(unsupported("company", name));
            }
        }
        if unsigned(world, b"PLYR", *id, "location_of_HQ")? != u64::from(u32::MAX) {
            return Err(unsupported("company", "headquarters"));
        }
        if !matches!(field(world,b"PLYR",*id,"name")?,WireValue::Bytes(v) if !v.is_empty()) {
            return Err(unsupported("company", "automatic naming"));
        }
        if signed(world, b"PLYR", *id, "money")? < 0
            || signed(world, b"PLYR", *id, "current_loan")? < 0
        {
            return Err(unsupported("company", "negative money or loan"));
        }
    }
    Ok(())
}
fn validate_towns(world: &World) -> Result<(), WorldTickError> {
    let towns = world
        .tables()
        .get(b"CITY")
        .ok_or_else(|| unsupported("town", "pool"))?;
    for id in towns.records().keys() {
        if unsigned(world, b"CITY", *id, "flags")? & 9 != 8
            || unsigned(world, b"CITY", *id, "growth_rate")? != 65535
            || unsigned(world, b"CITY", *id, "fund_buildings_months")? != 0
        {
            return Err(unsupported(
                "town",
                "growth must be custom-disabled and unfunded",
            ));
        }
    }
    Ok(())
}

fn validate_roads(world: &World) -> Result<bool, WorldTickError> {
    if !world
        .map()
        .tiles()
        .iter()
        .any(|tile| tile.tile_type() >> 4 == 2)
    {
        return Ok(false);
    }
    let towns = world
        .tables()
        .get(b"CITY")
        .ok_or_else(|| unsupported("road_tile_loop", "missing town pool"))?;
    for id in towns.records().keys() {
        if unsigned(world, b"CITY", *id, "road_build_months")? != 0 {
            return Err(unsupported("road_tile_loop", "town roadworks program"));
        }
    }
    for tile in world.map().tiles() {
        if !matches!(tile.tile_type() >> 4, 0 | 2 | 7) {
            return Err(unsupported(
                "road_tile_loop",
                "requires house-free clear/road/void map",
            ));
        }
        if tile.tile_type() >> 4 == 2
            && tile.m5() >> 6 == 0
            && !towns.records().contains_key(&u32::from(tile.m2()))
        {
            return Err(unsupported("road_tile_loop", "missing cached road town"));
        }
    }
    Ok(true)
}
