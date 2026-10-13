//! `CargoPayment` destructor income, expense ledger and vehicle profit publication.
use super::{State, VehicleId, WorldTickError, number, records, set, unsupported};
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};

pub(super) fn publish(
    state: &mut State<'_>,
    vehicle: VehicleId,
    payment: u32,
) -> Result<(), WorldTickError> {
    let visual = state.number(b"CAPY", payment, "visual_profit")?;
    let transfer = state.number(b"CAPY", payment, "visual_transfer")?;
    if visual == 0 && transfer == 0 {
        return Ok(());
    }
    let cost = state
        .number(b"CAPY", payment, "route_profit")?
        .saturating_neg();
    let owner = u32::try_from(number(state, vehicle, false, "owner")?)
        .map_err(|_| unsupported("cargo_payment", "company id"))?;
    if cost != 0 {
        state.set_number(
            b"PLYR",
            owner,
            "money",
            state.number(b"PLYR", owner, "money")?.saturating_sub(cost),
        )?;
        let WireValue::Array(expenses) = state.value(b"PLYR", owner, "yearly_expenses")? else {
            return Err(unsupported("cargo_payment", "expense array"));
        };
        let Some(WireValue::Signed(old)) = expenses.get(8) else {
            return Err(unsupported("cargo_payment", "road revenue slot"));
        };
        state.apply(WorldEdit::Field {
            chunk: *b"PLYR",
            record: owner,
            path: vec![
                PathElement::Field("yearly_expenses".into()),
                PathElement::Index(8),
            ],
            value: WireValue::Signed(old.saturating_add(cost)),
        })?;
        let economy = records::row(state, *b"PLYR", owner)?.child("cur_economy")?;
        let income = crate::world_access::row_field(economy.schema, economy.record, "income")?;
        let WireValue::Signed(income) = income else {
            return Err(unsupported("cargo_payment", "company income"));
        };
        state.apply(WorldEdit::Field {
            chunk: *b"PLYR",
            record: owner,
            path: vec![
                PathElement::Field("cur_economy".into()),
                PathElement::Index(0),
                PathElement::Field("income".into()),
            ],
            value: WireValue::Signed(income.saturating_sub(cost)),
        })?;
    }
    let profit = visual.saturating_add(transfer).wrapping_shl(8);
    set(
        state,
        vehicle,
        false,
        "profit_this_year",
        number(state, vehicle, false, "profit_this_year")?.saturating_add(profit),
    )
}
