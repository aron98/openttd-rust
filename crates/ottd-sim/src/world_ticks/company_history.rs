#[path = "company_service.rs"]
mod service;
use super::{WorldTickError, access::value_mut, unsupported};
use crate::WorldTickState as State;
use ottd_save::WireValue;
pub(super) fn quarter(
    state: &mut State<'_>,
    id: u32,
    phases: &mut impl crate::WorldTickPhases,
) -> Result<(), WorldTickError> {
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
    state.set(
        b"PLYR",
        id,
        "old_economy",
        WireValue::Structs(history.clone()),
    )?;
    state.set(
        b"PLYR",
        id,
        "cur_economy",
        WireValue::Structs(vec![cleared]),
    )?;
    let parts = super::company_rating::parts(state, id, service::facilities(state, id)?)?;
    let [.., score] = parts;
    phases.company_rating(id, parts)?;
    let assets = phases.company_assets(state, id)?;
    let first = history
        .first_mut()
        .ok_or_else(|| unsupported("company_quarter", "history"))?;
    *value_mut(&schema, first, "performance_history")? = WireValue::Signed(score);
    *value_mut(&schema, first, "company_value")? =
        WireValue::Signed(assets.saturating_sub(loan).saturating_add(money).max(1));
    state.set(b"PLYR", id, "old_economy", WireValue::Structs(history))?;
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

#[cfg(test)]
#[path = "company_quarter_fault.rs"]
mod fault;
