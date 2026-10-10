use super::{State, WorldTickError, unsupported};
pub(super) fn daily(state: &mut State<'_>) -> Result<(), WorldTickError> {
    let delay = state
        .number(b"DATE", 0, "next_disaster_start")?
        .wrapping_sub(1)
        & 0xffff;
    if delay == 0 {
        return Err(unsupported("disaster_day", "disaster delay reset/RNG"));
    }
    state.set_number(b"DATE", 0, "next_disaster_start", delay)?;
    let increment = u64::from(state.width).saturating_mul(u64::from(state.height)) / 31;
    let counter = u64::try_from(state.number(b"ECMY", 0, "industry_daily_change_counter")?)
        .map_err(|_| unsupported("industry_day", "counter"))?
        .wrapping_add(increment)
        & 0xffff_ffff;
    if counter >> 16 != 0 {
        return Err(unsupported(
            "industry_day",
            "industry production/construction attempt",
        ));
    }
    state.set_number(
        b"ECMY",
        0,
        "industry_daily_change_counter",
        i64::try_from(counter).map_err(|_| unsupported("industry_day", "counter"))?,
    )?;
    Ok(())
}
pub(super) fn competitor(state: &mut State<'_>) -> Result<(), WorldTickError> {
    if state.number(b"DATE", 0, "competitors_interval_fired")? != 0 {
        return Err(unsupported("company_tick", "competitor reset/script RNG"));
    }
    let period = state.number(b"DATE", 0, "competitors_interval")?;
    if period == 0 {
        return Ok(());
    }
    let elapsed = state
        .number(b"DATE", 0, "competitors_interval_elapsed")?
        .wrapping_add(1)
        & 0xffff_ffff;
    if elapsed >= period {
        return Err(unsupported("tick_timer", "competitor timeout"));
    }
    state.set_number(b"DATE", 0, "competitors_interval_elapsed", elapsed)
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
        industries: Vec::new(),
    };
    let result = crate::run_industry_month(
        callback,
        crate::IndustryMonth {
            month,
            year,
            days_since_last_month: days,
        },
    )?;
    state.set_number(b"IBLD", 0, "wanted_inds", i64::from(result.wanted_inds))
}
