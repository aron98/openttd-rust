// SPDX-License-Identifier: GPL-2.0-only
//! Typed forest cargo/history access preserving the remaining INDY record bytes.
use super::{State, TileArea, WOOD, WorldTickError, unsupported};
use crate::world_ticks::access::{number, value_mut};
use crate::{CallbackIndustry, IndustryProduced, ProducedHistory, world_access::row_field};
use ottd_save::{TableSchema, WireValue};

fn child<'a>(schema: &'a TableSchema, name: &str) -> Result<&'a TableSchema, WorldTickError> {
    schema
        .fields()
        .iter()
        .find(|f| f.name() == name)
        .and_then(ottd_save::FieldSchema::child)
        .ok_or_else(|| unsupported("industries", "child schema"))
}

pub(super) fn produced(state: &State<'_>, id: u32) -> Result<IndustryProduced, WorldTickError> {
    let table = state.table(b"INDY")?;
    let schema = child(table.schema(), "produced")?;
    let WireValue::Structs(rows) = state.value(b"INDY", id, "produced")? else {
        return Err(unsupported("industries", "produced rows"));
    };
    let [row] = rows.as_slice() else {
        return Err(unsupported("industries", "forest produced slots"));
    };
    let cargo = u8::try_from(number(row_field(schema, row, "cargo")?)?)
        .map_err(|_| unsupported("industries", "cargo"))?;
    if cargo != WOOD {
        return Err(unsupported("industries", "forest cargo specification"));
    }
    let WireValue::Structs(history) = row_field(schema, row, "history")? else {
        return Err(unsupported("industries", "history rows"));
    };
    if history.len() != 61 {
        return Err(unsupported("industries", "complete history required"));
    }
    let history_schema = child(schema, "history")?;
    let history = history
        .iter()
        .map(|row| {
            Ok(ProducedHistory {
                production: u16::try_from(number(row_field(history_schema, row, "production")?)?)
                    .map_err(|_| unsupported("industries", "production history"))?,
                transported: u16::try_from(number(row_field(history_schema, row, "transported")?)?)
                    .map_err(|_| unsupported("industries", "transported history"))?,
            })
        })
        .collect::<Result<Vec<_>, WorldTickError>>()?;
    Ok(IndustryProduced {
        cargo,
        waiting: u16::try_from(number(row_field(schema, row, "waiting")?)?)
            .map_err(|_| unsupported("industries", "waiting"))?,
        rate: u8::try_from(number(row_field(schema, row, "rate")?)?)
            .map_err(|_| unsupported("industries", "rate"))?,
        history,
    })
}

pub(super) fn set_produced(
    state: &mut State<'_>,
    id: u32,
    output: &IndustryProduced,
) -> Result<(), WorldTickError> {
    let schema = state.table(b"INDY")?.schema().clone();
    let schema = child(&schema, "produced")?;
    let WireValue::Structs(rows) = state.value(b"INDY", id, "produced")? else {
        return Err(unsupported("industries", "produced rows"));
    };
    let mut rows = rows.clone();
    let [row] = rows.as_mut_slice() else {
        return Err(unsupported("industries", "forest slots"));
    };
    *value_mut(schema, row, "waiting")? = WireValue::Unsigned(u64::from(output.waiting));
    *value_mut(schema, row, "rate")? = WireValue::Unsigned(u64::from(output.rate));
    let WireValue::Structs(history) = value_mut(schema, row, "history")? else {
        return Err(unsupported("industries", "history rows"));
    };
    if history.len() != output.history.len() {
        return Err(unsupported("industries", "history length"));
    }
    let schema = child(schema, "history")?;
    for (row, value) in history.iter_mut().zip(&output.history) {
        *value_mut(schema, row, "production")? = WireValue::Unsigned(u64::from(value.production));
        *value_mut(schema, row, "transported")? = WireValue::Unsigned(u64::from(value.transported));
    }
    state.set(b"INDY", id, "produced", WireValue::Structs(rows))
}

pub(super) fn area(state: &State<'_>, id: u32) -> Result<TileArea, WorldTickError> {
    Ok(TileArea {
        tile: u32::try_from(state.number(b"INDY", id, "location.tile")?)
            .map_err(|_| unsupported("industries", "location"))?,
        width: u16::try_from(state.number(b"INDY", id, "location.w")?)
            .map_err(|_| unsupported("industries", "width"))?,
        height: u16::try_from(state.number(b"INDY", id, "location.h")?)
            .map_err(|_| unsupported("industries", "height"))?,
    })
}

pub(in crate::world_ticks) fn monthly_state(
    state: &State<'_>,
) -> Result<Vec<CallbackIndustry>, WorldTickError> {
    state
        .table(b"INDY")?
        .records()
        .map(|(id, _)| {
            Ok(CallbackIndustry {
                id: u16::try_from(id).map_err(|_| unsupported("industry_month", "ID"))?,
                industry_type: u8::try_from(state.number(b"INDY", id, "type")?)
                    .map_err(|_| unsupported("industry_month", "type"))?,
                prod_level: u8::try_from(state.number(b"INDY", id, "prod_level")?)
                    .map_err(|_| unsupported("industry_month", "production level"))?,
                last_prod_year: i32::try_from(state.number(b"INDY", id, "last_prod_year")?)
                    .map_err(|_| unsupported("industry_month", "production year"))?,
                valid_history: match state.value(b"INDY", id, "valid_history")? {
                    WireValue::Unsigned(value) => *value,
                    _ => return Err(unsupported("industry_month", "valid history mask")),
                },
                produced: vec![produced(state, id)?],
                accepted: Vec::new(),
            })
        })
        .collect()
}

pub(in crate::world_ticks) fn publish_month(
    state: &mut State<'_>,
    industries: &[CallbackIndustry],
) -> Result<(), WorldTickError> {
    for industry in industries {
        let id = u32::from(industry.id);
        state.set_number(
            b"INDY",
            id,
            "last_prod_year",
            i64::from(industry.last_prod_year),
        )?;
        state.set(
            b"INDY",
            id,
            "valid_history",
            WireValue::Unsigned(industry.valid_history),
        )?;
        let [output] = industry.produced.as_slice() else {
            return Err(unsupported("industry_month", "forest produced slots"));
        };
        set_produced(state, id, output)?;
    }
    Ok(())
}
