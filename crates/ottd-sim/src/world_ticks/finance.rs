use super::{
    WorldTickError,
    access::{State, number, value_mut},
    unsupported,
};
use crate::world_access::unsigned;
use ottd_save::{WireValue, world::World};

pub(super) fn limits(state: &mut State, world: &World) -> Result<(), WorldTickError> {
    for id in state.company_ids() {
        for kind in ["terraform", "clear", "tree"] {
            let name = format!("{kind}_limit");
            let rate = unsigned(
                world,
                b"PATS",
                0,
                &format!("construction.{kind}_per_64k_frames"),
            )?;
            let cap = unsigned(
                world,
                b"PATS",
                0,
                &format!("construction.{kind}_frame_burst"),
            )? << 16;
            let previous = u64::try_from(state.number(b"PLYR", id, &name)?)
                .map_err(|_| unsupported("limits", &name))?;
            let value = previous.saturating_add(rate).min(cap);
            state.set_number(
                b"PLYR",
                id,
                &name,
                i64::try_from(value).map_err(|_| unsupported("limits", &name))?,
            )?;
        }
    }
    Ok(())
}
pub(super) fn monthly(state: &mut State, world: &World, month: u8) -> Result<(), WorldTickError> {
    for id in state.company_ids() {
        solvent(state, world, id)?;
        state.set_number(b"PLYR", id, "months_of_bankruptcy", 0)?;
        state.set_number(b"PLYR", id, "bankrupt_asked", 0)?;
        if month % 3 == 0 {
            super::company_history::quarter(state, id)?;
        }
    }
    let inflation = unsigned(world, b"ECMY", 0, "inflation_prices")?;
    let overhead = i64::try_from(
        (inflation
            .checked_mul(100)
            .ok_or_else(|| unsupported("company_month", "price overflow"))?
            >> 16)
            .max(1)
            >> 2,
    )
    .map_err(|_| unsupported("company_month", "price range"))?;
    let rate = state.number(b"ECMY", 0, "interest_rate")?;
    for id in state.company_ids() {
        let loan = state.number(b"PLYR", id, "current_loan")?;
        let annual = loan.saturating_mul(rate) / 100;
        let previous = annual.saturating_mul(i64::from(month)) / 12;
        let next = annual.saturating_mul(i64::from(month).saturating_add(1)) / 12;
        debit(
            state,
            id,
            11,
            next.checked_sub(previous)
                .ok_or_else(|| unsupported("company_month", "interest overflow"))?,
        )?;
        debit(state, id, 12, overhead)?;
        if state.number(b"PLYR", id, "money")? < 0 {
            return Err(unsupported("company_month", "projected negative cash"));
        }
    }
    Ok(())
}
fn solvent(state: &State, world: &World, id: u32) -> Result<(), WorldTickError> {
    if unsigned(world, b"PATS", 0, "difficulty.infinite_money")? != 0 {
        return Ok(());
    }
    let personal = state.number(b"PLYR", id, "max_loan")?;
    let maximum = if personal == i64::MIN {
        let base = unsigned(world, b"PATS", 0, "difficulty.max_loan")?;
        let inflation = unsigned(world, b"ECMY", 0, "inflation_prices")?;
        i64::try_from(
            ((base
                .checked_mul(inflation)
                .ok_or_else(|| unsupported("company_month", "loan inflation overflow"))?
                >> 16)
                / 10_000)
                .saturating_mul(10_000),
        )
        .map_err(|_| unsupported("company_month", "loan range"))?
    } else {
        personal
    };
    let cash = i128::from(state.number(b"PLYR", id, "money")?);
    let loan = i128::from(state.number(b"PLYR", id, "current_loan")?);
    if cash.saturating_sub(loan) < i128::from(maximum).saturating_neg() {
        return Err(unsupported("company_month", "bankruptcy lifecycle"));
    }
    Ok(())
}

fn debit(state: &mut State, id: u32, expense: usize, cost: i64) -> Result<(), WorldTickError> {
    if cost == 0 {
        return Ok(());
    }
    state.set_number(
        b"PLYR",
        id,
        "money",
        state
            .number(b"PLYR", id, "money")?
            .checked_sub(cost)
            .ok_or_else(|| unsupported("company_month", "cash overflow"))?,
    )?;
    let WireValue::Array(mut expenses) = state.value(b"PLYR", id, "yearly_expenses")?.clone()
    else {
        return Err(unsupported("company_month", "expense array"));
    };
    let slot = expenses
        .get_mut(expense)
        .ok_or_else(|| unsupported("company_month", "expense slot"))?;
    *slot = WireValue::Signed(
        number(slot)?
            .checked_add(cost)
            .ok_or_else(|| unsupported("company_month", "expense overflow"))?,
    );
    state.set(b"PLYR", id, "yearly_expenses", WireValue::Array(expenses))?;
    if expense == 11 {
        let table = state
            .tables
            .get(b"PLYR")
            .ok_or_else(|| unsupported("company_month", "pool"))?;
        let schema = table
            .schema()
            .fields()
            .iter()
            .find(|f| f.name() == "cur_economy")
            .and_then(|f| f.child())
            .ok_or_else(|| unsupported("company_month", "current schema"))?
            .clone();
        let WireValue::Structs(mut current) = state.value(b"PLYR", id, "cur_economy")?.clone()
        else {
            return Err(unsupported("company_month", "current economy"));
        };
        let slot = value_mut(
            &schema,
            current
                .first_mut()
                .ok_or_else(|| unsupported("company_month", "current economy"))?,
            "expenses",
        )?;
        *slot = WireValue::Signed(
            number(slot)?
                .checked_sub(cost)
                .ok_or_else(|| unsupported("company_month", "current expense overflow"))?,
        );
        state.set(b"PLYR", id, "cur_economy", WireValue::Structs(current))?;
    }
    Ok(())
}
pub(super) fn yearly(state: &mut State) -> Result<(), WorldTickError> {
    let mut companies = Vec::new();
    for id in state.company_ids() {
        let WireValue::Array(values) = state.value(b"PLYR", id, "yearly_expenses")? else {
            return Err(unsupported("company_year", "expenses"));
        };
        let mut yearly_expenses = [[0_i64; 13]; 3];
        for (target, value) in yearly_expenses.iter_mut().flatten().zip(values) {
            *target = number(value)?;
        }
        companies.push(crate::CompanyExpenses {
            id: u8::try_from(id).map_err(|_| unsupported("company_year", "id"))?,
            yearly_expenses,
        });
    }
    let result = crate::run_company_year(crate::CompanyCallbacks {
        show_finances: false,
        random_state: state.random_state()?,
        companies,
    })?;
    for company in result.companies {
        state.set(
            b"PLYR",
            u32::from(company.id),
            "yearly_expenses",
            WireValue::Array(
                company
                    .yearly_expenses
                    .into_iter()
                    .flatten()
                    .map(WireValue::Signed)
                    .collect(),
            ),
        )?;
    }
    Ok(())
}
