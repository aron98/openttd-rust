use super::{
    WorldTickError,
    access::{number, value_mut},
    unsupported,
};
use crate::WorldTickState as State;
use crate::world_access::row_field;
use ottd_save::{TableRecord, TableSchema, WireValue};
fn checked(value: i128) -> Result<i64, WorldTickError> {
    i64::try_from(value).map_err(|_| unsupported("company_quarter", "accounting overflow"))
}
pub(super) fn quarter(state: &mut State<'_>, id: u32) -> Result<(), WorldTickError> {
    let table = state.table(b"PLYR")?;
    let schema = table
        .schema()
        .fields()
        .iter()
        .find(|f| f.name() == "old_economy")
        .and_then(|f| f.child())
        .ok_or_else(|| unsupported("company_quarter", "history schema"))?
        .clone();
    let WireValue::Structs(current) = state.value(b"PLYR", id, "cur_economy")? else {
        return Err(unsupported("company_quarter", "current economy"));
    };
    let entry = current
        .first()
        .ok_or_else(|| unsupported("company_quarter", "missing current economy"))?
        .clone();
    let mut cleared = entry.clone();
    for value in cleared.values_mut() {
        zero(value)?;
    }
    let WireValue::Structs(old) = state.value(b"PLYR", id, "old_economy")? else {
        return Err(unsupported("company_quarter", "old economy"));
    };
    let mut history = old.clone();
    history.insert(0, entry);
    history.truncate(24);
    let money = state.number(b"PLYR", id, "money")?;
    let loan = state.number(b"PLYR", id, "current_loan")?;
    let score = rating(&schema, &history, money, loan)?;
    let first = history
        .first_mut()
        .ok_or_else(|| unsupported("company_quarter", "history"))?;
    *value_mut(&schema, first, "performance_history")? = WireValue::Signed(score);
    *value_mut(&schema, first, "company_value")? = WireValue::Signed(
        money
            .checked_sub(loan)
            .ok_or_else(|| unsupported("company_quarter", "company value overflow"))?
            .max(1),
    );
    state.set(b"PLYR", id, "old_economy", WireValue::Structs(history))?;
    state.set(
        b"PLYR",
        id,
        "cur_economy",
        WireValue::Structs(vec![cleared]),
    )?;
    state.set_number(
        b"PLYR",
        id,
        "block_preview",
        state
            .number(b"PLYR", id, "block_preview")?
            .saturating_sub(1)
            .max(0),
    )
}
fn zero(value: &mut WireValue) -> Result<(), WorldTickError> {
    match value {
        WireValue::Signed(v) => *v = 0,
        WireValue::Unsigned(v) => *v = 0,
        WireValue::Array(values) => {
            for v in values {
                zero(v)?;
            }
        }
        WireValue::Bytes(_) | WireValue::Structs(_) => {
            return Err(unsupported("company_quarter", "unexpected economy field"));
        }
    }
    Ok(())
}
fn rating(
    schema: &TableSchema,
    history: &[TableRecord],
    money: i64,
    loan: i64,
) -> Result<i64, WorldTickError> {
    let mut incomes = Vec::new();
    for row in history.iter().take(12) {
        incomes.push(checked(
            i128::from(number(row_field(schema, row, "income")?)?)
                .saturating_add(i128::from(number(row_field(schema, row, "expenses")?)?)),
        )?);
    }
    let min = incomes.iter().copied().min().unwrap_or(0);
    let max = incomes.iter().copied().max().unwrap_or(0);
    let mut delivered = 0_i128;
    let mut variety = 0_i64;
    for (index, row) in history.iter().take(4).enumerate() {
        let WireValue::Array(cargo) = row_field(schema, row, "delivered_cargo")? else {
            return Err(unsupported("company_quarter", "cargo history"));
        };
        for value in cargo {
            let count = number(value)?;
            delivered = delivered.saturating_add(i128::from(count));
            if index == 0 && count != 0 {
                variety = variety.saturating_add(1);
            }
        }
    }
    let mut score = 0_i64;
    for (value, needed, weight) in [
        (min, 50_000, 50),
        (max, 100_000, 100),
        (checked(delivered)?, 40_000, 400),
        (variety, 8, 50),
        (money, 10_000_000, 50),
        (250_000_i64.saturating_sub(loan), 250_000, 50),
    ] {
        score = score.saturating_add(
            value
                .clamp(0, needed)
                .saturating_mul(weight)
                .checked_div(needed)
                .ok_or_else(|| unsupported("company_quarter", "score divisor"))?,
        );
    }
    Ok(score)
}
