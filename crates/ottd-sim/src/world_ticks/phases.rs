use super::{State, WorldTickError, unsupported};
pub(super) fn daily(state: &mut State<'_>) -> Result<(), WorldTickError> {
    let delay = state
        .number(b"DATE", 0, "next_disaster_start")?
        .wrapping_sub(1)
        & 0xffff;
    let delay = if delay == 0 {
        i64::from((super::house_cargo::random(state)? & 0x1ff).wrapping_add(730))
    } else {
        delay
    };
    state.set_number(b"DATE", 0, "next_disaster_start", delay)?;
    super::industry_manager::daily(state)
}

pub(super) fn competitor(state: &mut State<'_>) -> Result<(), WorldTickError> {
    if state.number(b"DATE", 0, "competitors_interval_fired")? != 0 {
        return Ok(());
    }
    let period = state.number(b"DATE", 0, "competitors_interval")?;
    if period == 0 {
        return Ok(());
    }
    let elapsed = state
        .number(b"DATE", 0, "competitors_interval_elapsed")?
        .wrapping_add(1)
        & 0xffff_ffff;
    state.set_number(b"DATE", 0, "competitors_interval_elapsed", elapsed)?;
    if elapsed >= period {
        state.set_number(b"DATE", 0, "competitors_interval_fired", 1)?;
    }
    Ok(())
}

pub(super) fn company_tick(state: &mut State<'_>) -> Result<(), WorldTickError> {
    super::company_name::tick(state)?;
    if state.number(b"DATE", 0, "competitors_interval_fired")? == 0 {
        return Ok(());
    }
    if state.number(b"PATS", 0, "difficulty.max_no_competitors")? != 0 {
        return Err(unsupported("company_tick", "new AI company"));
    }
    let interval = u32::try_from(state.number(b"PATS", 0, "difficulty.competitors_interval")?)
        .map_err(|_| unsupported("company_tick", "competitor interval"))?;
    let seconds = if interval == 0 {
        600
    } else {
        interval
            .checked_mul(60)
            .ok_or_else(|| unsupported("company_tick", "competitor interval overflow"))?
    };
    let timeout = seconds
        .checked_mul(37)
        .ok_or_else(|| unsupported("company_tick", "competitor timeout overflow"))?;
    let mut random = ottd_core::Randomizer::from_state([
        u32::try_from(state.number(b"SRND", 16, "state[0]")?)
            .map_err(|_| unsupported("company_tick", "script random word"))?,
        u32::try_from(state.number(b"SRND", 16, "state[1]")?)
            .map_err(|_| unsupported("company_tick", "script random word"))?,
    ]);
    let jitter = random.next_bounded(timeout / 4);
    for (name, value) in ["state[0]", "state[1]"].into_iter().zip(random.state()) {
        state.set_number(b"SRND", 16, name, i64::from(value))?;
    }
    let timeout = timeout
        .checked_add(jitter)
        .and_then(|value| value.checked_sub(timeout / 8))
        .ok_or_else(|| unsupported("company_tick", "competitor jitter overflow"))?
        .max(1);
    state.set_number(b"DATE", 0, "competitors_interval", i64::from(timeout))?;
    state.set_number(b"DATE", 0, "competitors_interval_elapsed", 0)?;
    state.set_number(b"DATE", 0, "competitors_interval_fired", 0)
}
pub(super) fn industry_month(
    state: &mut State<'_>,
    month: u8,
    year: i32,
    days: u32,
) -> Result<(), WorldTickError> {
    let callback = crate::IndustryCallbacks {
        economy_type: 0,
        newgrf: false,
        industry_density: u8::try_from(state.unsigned(
            b"PATS",
            0,
            "difficulty.industry_density",
        )?)
        .map_err(|_| unsupported("industry_month", "density"))?,
        map_width: state.width,
        map_height: state.height,
        current_company: 255,
        random_state: state.random_state()?,
        wanted_inds: u32::try_from(state.number(b"IBLD", 0, "wanted_inds")?)
            .map_err(|_| unsupported("industry_month", "builder"))?,
        industries: super::industries::monthly_state(state)?,
    };
    let result = crate::run_industry_month(
        callback,
        crate::IndustryMonth {
            month,
            year,
            days_since_last_month: days,
        },
    )?;
    state.set_number(b"IBLD", 0, "wanted_inds", i64::from(result.wanted_inds))?;
    super::industries::publish_month(state, &result.industries)
}

#[cfg(test)]
#[path = "disaster_tests.rs"]
mod disaster_tests;
