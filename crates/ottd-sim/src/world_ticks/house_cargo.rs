// SPDX-License-Identifier: GPL-2.0-only
//! `TownGenerateCargo` production and the explicitly empty `StationFinder` branch.

use super::{
    State, WorldTickError,
    access::{number, value_mut},
    unsupported,
};
use crate::world_access::row_field;
use ottd_save::{TableRecord, TableSchema, WireValue};

#[derive(Debug, Clone, Copy)]
pub(crate) enum CargoSource {
    Town(u16),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TileArea {
    pub tile: u32,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TownCargoProduction {
    pub source: CargoSource,
    pub area: TileArea,
    pub cargo: u8,
    pub amount: u32,
}

pub(super) fn random(state: &mut State<'_>) -> Result<u32, WorldTickError> {
    let mut rng = ottd_core::Randomizer::from_state(state.random_state()?);
    let draw = rng.next_u32();
    for (name, word) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(rng.state())
    {
        state.set_number(b"DATE", 0, name, i64::from(word))?;
    }
    Ok(draw)
}

fn scaled(state: &mut State<'_>, amount: u32) -> Result<u32, WorldTickError> {
    let percentage = u32::try_from(state.number(b"PATS", 0, "economy.town_cargo_scale")?)
        .map_err(|_| unsupported("house_cargo", "cargo scale"))?;
    if !(15..=300).contains(&percentage) {
        return Err(unsupported(
            "house_cargo",
            "cargo scale outside native settings",
        ));
    }
    if percentage == 100 {
        return Ok(amount);
    }
    let scaled = amount.wrapping_mul(percentage) / 100;
    if scaled != 0 {
        return Ok(scaled);
    }
    let denominator = 100_u32
        .checked_div(percentage)
        .ok_or_else(|| unsupported("house_cargo", "zero cargo scale"))?
        .max(1);
    let chance = (random(state)? & 0xffff)
        .wrapping_mul(denominator)
        .wrapping_add(denominator / 2)
        >> 16;
    Ok(u32::from(chance < 1))
}

pub(super) fn generate(
    state: &mut State<'_>,
    mut production: TownCargoProduction,
    delivery: &mut impl crate::WorldTickPhases,
) -> Result<(), WorldTickError> {
    if production.amount == 0 {
        return Ok(());
    }
    if state.number(b"ECMY", 0, "fluct")? <= 0 {
        production.amount = production.amount.wrapping_add(1) >> 1;
    }
    production.amount = scaled(state, production.amount)?;
    if production.amount == 0 {
        return Ok(());
    }
    publish(state, production, 0)?;
    let delivered = delivery.town_cargo(state, production)?;
    if delivered != 0 {
        production.amount = 0;
        publish(state, production, delivered)?;
    }
    Ok(())
}

pub(crate) fn deliver_empty(
    state: &State<'_>,
    production: TownCargoProduction,
) -> Result<u32, WorldTickError> {
    if state.table(b"STNN")?.records().next().is_some() {
        return Err(unsupported(
            "house_cargo",
            "station distribution and packet allocation",
        ));
    }
    let CargoSource::Town(town) = production.source;
    let tile = state.view().tile(production.area.tile)?;
    if production.area.width != 1
        || production.area.height != 1
        || tile.tile_type() >> 4 != 3
        || tile.m2() != town
    {
        return Err(unsupported("house_cargo", "producer source or tile area"));
    }
    Ok(0)
}

fn child<'a>(schema: &'a TableSchema, name: &str) -> Result<&'a TableSchema, WorldTickError> {
    schema
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .and_then(ottd_save::FieldSchema::child)
        .ok_or_else(|| unsupported("house_cargo", "child schema"))
}

fn empty_history(schema: &TableSchema) -> Result<TableRecord, WorldTickError> {
    let mut values = Vec::new();
    for field in schema.fields() {
        if !matches!(field.name(), "production" | "transported") {
            return Err(unsupported("house_cargo", "history schema"));
        }
        values.push(WireValue::Unsigned(0));
    }
    Ok(TableRecord::new(values))
}

fn publish(
    state: &mut State<'_>,
    production: TownCargoProduction,
    delivered: u32,
) -> Result<(), WorldTickError> {
    let CargoSource::Town(town) = production.source;
    let schema = state.table(b"CITY")?.schema().clone();
    let supplied_schema = child(&schema, "supplied")?;
    let history_schema = child(supplied_schema, "history")?;
    let WireValue::Structs(existing) = state.value(b"CITY", u32::from(town), "supplied")? else {
        return Err(unsupported("house_cargo", "supplied rows"));
    };
    let mut supplied = existing.clone();
    let mut position = 0;
    while let Some(row) = supplied.get(position) {
        if number(row_field(supplied_schema, row, "cargo")?)? >= i64::from(production.cargo) {
            break;
        }
        position = position
            .checked_add(1)
            .ok_or_else(|| unsupported("house_cargo", "supplied position overflow"))?;
    }
    let exists = supplied
        .get(position)
        .map(|row| number(row_field(supplied_schema, row, "cargo")?))
        .transpose()?
        .is_some_and(|id| id == i64::from(production.cargo));
    if !exists {
        let mut values = Vec::new();
        for field in supplied_schema.fields() {
            values.push(match field.name() {
                "cargo" => WireValue::Unsigned(u64::from(production.cargo)),
                "history" => WireValue::Structs(vec![empty_history(history_schema)?; 61]),
                _ => return Err(unsupported("house_cargo", "supplied schema")),
            });
        }
        supplied.insert(position, TableRecord::new(values));
    }
    let row = supplied
        .get_mut(position)
        .ok_or_else(|| unsupported("house_cargo", "supplied position"))?;
    let WireValue::Structs(history) = value_mut(supplied_schema, row, "history")? else {
        return Err(unsupported("house_cargo", "history rows"));
    };
    let current = history
        .first_mut()
        .ok_or_else(|| unsupported("house_cargo", "current history"))?;
    for (name, amount) in [
        ("production", production.amount),
        ("transported", delivered),
    ] {
        let value = value_mut(history_schema, current, name)?;
        let old = u32::try_from(number(value)?)
            .map_err(|_| unsupported("house_cargo", "history range"))?;
        *value = WireValue::Unsigned(u64::from(old.wrapping_add(amount)));
    }
    state.set(
        b"CITY",
        u32::from(town),
        "supplied",
        WireValue::Structs(supplied),
    )
}
