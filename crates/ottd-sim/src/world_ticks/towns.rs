use super::{
    WorldTickError,
    access::{number, value_mut},
    unsupported,
};
use crate::WorldTickState as State;
use crate::world_access::row_field;
use ottd_save::{TableRecord, TableSchema, WireValue};

pub(super) fn monthly(state: &mut State<'_>, month: u8) -> Result<(), WorldTickError> {
    let companies = state.company_ids();
    let schema = state.table(b"CITY")?.schema().clone();
    let ids: Vec<_> = state.table(b"CITY")?.records().map(|(id, _)| id).collect();
    for id in ids {
        let mut record = state
            .table(b"CITY")?
            .record(id)
            .ok_or_else(|| unsupported("town_month", "town"))?
            .clone();
        let row = &mut record;
        let exclusive_was_active = number(row_field(&schema, row, "exclusive_counter")?)? > 0;
        for name in ["road_build_months", "exclusive_counter"] {
            let value = value_mut(&schema, row, name)?;
            *value = WireValue::Unsigned(
                u64::try_from(number(value)?.saturating_sub(1).max(0))
                    .map_err(|_| unsupported("town_month", name))?,
            );
        }
        if exclusive_was_active && number(row_field(&schema, row, "exclusive_counter")?)? == 0 {
            *value_mut(&schema, row, "exclusivity")? = WireValue::Unsigned(255);
        }
        let WireValue::Array(unwanted) = value_mut(&schema, row, "unwanted")? else {
            return Err(unsupported("town_month", "unwanted array"));
        };
        for id in &companies {
            let v = unwanted
                .get_mut(usize::try_from(*id).map_err(|_| unsupported("town_month", "company id"))?)
                .ok_or_else(|| unsupported("town_month", "unwanted slot"))?;
            let count = number(v)?;
            if count > 0 {
                *v = WireValue::Signed(count.saturating_sub(1));
            }
        }
        let WireValue::Array(ratings) = value_mut(&schema, row, "ratings")? else {
            return Err(unsupported("town_month", "ratings array"));
        };
        for (index, value) in ratings.iter_mut().enumerate() {
            let mut rating = number(value)?;
            if companies
                .iter()
                .any(|id| usize::try_from(*id).ok() == Some(index))
                && rating < 200
            {
                rating = rating.saturating_add(5).min(200);
            }
            *value = WireValue::Signed(rating.clamp(-1000, 1000));
        }
        let WireValue::Unsigned(valid) = row_field(&schema, row, "valid_history")? else {
            return Err(unsupported("town_month", "history mask"));
        };
        let mask = crate::industry_history::update_valid(*valid, month)?;
        *value_mut(&schema, row, "valid_history")? = WireValue::Unsigned(mask);
        supplied(&schema, row, mask, month)?;
        received(&schema, row)?;
        state.apply(ottd_save::world::WorldEdit::ReplaceRecord {
            chunk: *b"CITY",
            record: id,
            value: record,
        })?;
    }
    Ok(())
}
fn child<'a>(schema: &'a TableSchema, name: &str) -> Result<&'a TableSchema, WorldTickError> {
    schema
        .fields()
        .iter()
        .find(|f| f.name() == name)
        .and_then(|f| f.child())
        .ok_or_else(|| unsupported("town_month", name))
}
fn received(schema: &TableSchema, row: &mut TableRecord) -> Result<(), WorldTickError> {
    let schema_child = child(schema, "received")?;
    let WireValue::Structs(rows) = value_mut(schema, row, "received")? else {
        return Err(unsupported("town_month", "received"));
    };
    for row in rows {
        for (old, new) in [("old_max", "new_max"), ("old_act", "new_act")] {
            *value_mut(schema_child, row, old)? = row_field(schema_child, row, new)?.clone();
            *value_mut(schema_child, row, new)? = WireValue::Unsigned(0);
        }
    }
    Ok(())
}
fn supplied(
    schema: &TableSchema,
    row: &mut TableRecord,
    mask: u64,
    month: u8,
) -> Result<(), WorldTickError> {
    let supplied_schema = child(schema, "supplied")?;
    let history_schema = child(supplied_schema, "history")?;
    let WireValue::Structs(supplied) = value_mut(schema, row, "supplied")? else {
        return Err(unsupported("town_month", "supplied"));
    };
    for cargo in supplied {
        let WireValue::Structs(history) = value_mut(supplied_schema, cargo, "history")? else {
            return Err(unsupported("town_month", "history"));
        };
        if history.len() != 61 {
            return Err(unsupported("town_month", "incomplete supplied history"));
        }
        for (first, last, division, total) in [
            (1_usize, 25_usize, 1_usize, 1_u8),
            (25, 42, 3, 3),
            (42, 61, 4, 12),
        ] {
            if month
                .checked_rem(total)
                .ok_or_else(|| unsupported("town_month", "history interval"))?
                != 0
            {
                continue;
            }
            let range = history
                .get_mut(first..last)
                .ok_or_else(|| unsupported("town_month", "history range"))?;
            let previous = range
                .first()
                .ok_or_else(|| unsupported("town_month", "history range"))?
                .clone();
            range.rotate_right(1);
            *range
                .first_mut()
                .ok_or_else(|| unsupported("town_month", "history range"))? = previous;
            if total == 1 {
                let current = history
                    .first()
                    .ok_or_else(|| unsupported("town_month", "current history"))?
                    .clone();
                *history
                    .get_mut(first)
                    .ok_or_else(|| unsupported("town_month", "history slot"))? = current;
                let current = history
                    .first_mut()
                    .ok_or_else(|| unsupported("town_month", "current history"))?;
                for name in ["production", "transported"] {
                    *value_mut(history_schema, current, name)? = WireValue::Unsigned(0);
                }
            } else if mask & (1_u64 << first.saturating_sub(division)) != 0 {
                for name in ["production", "transported"] {
                    // Native std::accumulate(..., 0, ...) uses int32, including its final sign extension.
                    let mut sum = 0_u32;
                    for source in history
                        .get(first.saturating_sub(division)..first)
                        .ok_or_else(|| unsupported("town_month", "history source"))?
                    {
                        sum = sum.wrapping_add(
                            u32::try_from(number(row_field(history_schema, source, name)?)?)
                                .map_err(|_| unsupported("town_month", "history value"))?,
                        );
                    }
                    let signed_sum = i32::from_ne_bytes(sum.to_ne_bytes());
                    let native_sum = u64::from_ne_bytes(i64::from(signed_sum).to_ne_bytes());
                    let average = native_sum
                        .checked_div(
                            u64::try_from(division)
                                .map_err(|_| unsupported("town_month", "history division"))?,
                        )
                        .ok_or_else(|| unsupported("town_month", "history division"))?;
                    *value_mut(
                        history_schema,
                        history
                            .get_mut(first)
                            .ok_or_else(|| unsupported("town_month", "history target"))?,
                        name,
                    )? = WireValue::Unsigned(average.min(u64::from(u32::MAX)));
                }
            }
        }
    }
    Ok(())
}
