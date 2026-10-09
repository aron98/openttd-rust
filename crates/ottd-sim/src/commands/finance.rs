use super::{CommandCost, CommandError, Plan};
use crate::world_access::{field_edit, signed, unsigned};
use ottd_save::{WireValue, world::World};

pub(super) fn loan(
    world: &World,
    company: u8,
    args: (u8, i64),
    increase: bool,
) -> Result<Plan, CommandError> {
    let company = u32::from(company);
    let (method, amount) = args;
    let current = signed(world, b"PLYR", company, "current_loan")?;
    let money = signed(world, b"PLYR", company, "money")?;
    let infinite = unsigned(world, b"PATS", 0, "difficulty.infinite_money")? != 0;
    let available = if infinite { i64::MAX } else { money };
    let maximum = max_loan(world, company)?;
    if increase && current >= maximum {
        return Ok(Plan::empty(CommandCost::parameterized(
            "STR_ERROR_MAXIMUM_PERMITTED_LOAN",
            maximum,
        )));
    }
    if !increase && current == 0 {
        return Ok(Plan::empty(CommandCost::failure(
            "STR_ERROR_LOAN_ALREADY_REPAID",
        )));
    }
    let value = match method {
        0 => {
            if increase {
                10_000
            } else {
                current.min(10_000)
            }
        }
        1 => {
            if increase {
                maximum
                    .checked_sub(current)
                    .ok_or(CommandError::Overflow("maximum loan"))?
            } else {
                let value = current.min(available).max(10_000);
                value
                    .checked_sub(value % 10_000)
                    .ok_or(CommandError::Overflow("loan rounding"))?
            }
        }
        2 => {
            if amount < 10_000
                || amount % 10_000 != 0
                || (increase && current.checked_add(amount).is_none_or(|n| n > maximum))
                || (!increase && amount > current)
            {
                return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
            }
            amount
        }
        _ => return Ok(Plan::empty(CommandCost::failure("CMD_ERROR"))),
    };
    if increase && money.checked_add(value).is_none() {
        return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
    }
    if !increase && available < value {
        return Ok(Plan::empty(CommandCost::parameterized(
            "STR_ERROR_CURRENCY_REQUIRED",
            value,
        )));
    }
    let next_money = if increase {
        money.checked_add(value)
    } else {
        money.checked_sub(value)
    }
    .ok_or(CommandError::Overflow("loan cash"))?;
    let next_loan = if increase {
        current.checked_add(value)
    } else {
        current.checked_sub(value)
    }
    .ok_or(CommandError::Overflow("loan balance"))?;
    Ok(Plan {
        cost: CommandCost::success(0, if increase { 12 } else { 255 }),
        edits: vec![
            field_edit(*b"PLYR", company, "money", WireValue::Signed(next_money)),
            field_edit(
                *b"PLYR",
                company,
                "current_loan",
                WireValue::Signed(next_loan),
            ),
        ],
    })
}
fn max_loan(world: &World, company: u32) -> Result<i64, CommandError> {
    let personal = signed(world, b"PLYR", company, "max_loan")?;
    if personal != i64::MIN {
        return Ok(personal);
    }
    let base = unsigned(world, b"PATS", 0, "difficulty.max_loan")?;
    let inflation = unsigned(world, b"ECMY", 0, "inflation_prices")?;
    let inflated = base
        .checked_mul(inflation)
        .ok_or(CommandError::Overflow("inflated loan"))?
        >> 16;
    let rounded = inflated
        .checked_sub(inflated % 10_000)
        .ok_or(CommandError::Overflow("loan rounding"))?;
    i64::try_from(rounded).map_err(|_| CommandError::Overflow("maximum loan"))
}
