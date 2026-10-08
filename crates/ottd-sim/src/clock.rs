use crate::{Clock, Context, SimulationError};
use ottd_core::{
    CalendarDate, ClockCache, ClockSettings, ClockSnapshot, ClockState, DateFraction, EconomyDate,
    TickCounter, TimekeepingUnits,
};

#[expect(
    clippy::redundant_pub_crate,
    reason = "crate-private adapter is intentionally not part of the public API"
)]
pub(crate) fn restore(clock: &Clock, context: &Context) -> Result<ClockState, SimulationError> {
    let units = match context.timekeeping_units {
        0 => TimekeepingUnits::Calendar,
        1 => TimekeepingUnits::Wallclock,
        _ => return Err(SimulationError::Context),
    };
    let settings = ClockSettings::new(units, context.minutes_per_calendar_year)?;
    let snapshot = ClockSnapshot {
        date: CalendarDate::from_raw(clock.date).map_err(|_| SimulationError::CalendarDate)?,
        date_fract: DateFraction(clock.date_fract),
        calendar_sub_date_fract: clock.calendar_sub_date_fract,
        economy_date: EconomyDate(clock.economy_date),
        economy_date_fract: DateFraction(clock.economy_date_fract),
        days_since_last_month: clock.days_since_last_month,
        tick_counter: TickCounter(clock.tick_counter),
    };
    Ok(ClockState::restore(
        snapshot,
        settings,
        ClockCache {
            calendar_year: clock.calendar_year,
            calendar_month: clock.calendar_month,
            economy_year: clock.economy_year,
            economy_month: clock.economy_month,
        },
    )?)
}

impl From<ClockState> for Clock {
    fn from(state: ClockState) -> Self {
        let saved = state.snapshot();
        let cache = state.cache();
        Self {
            date: saved.date.raw(),
            date_fract: saved.date_fract.0,
            calendar_sub_date_fract: saved.calendar_sub_date_fract,
            economy_date: saved.economy_date.0,
            economy_date_fract: saved.economy_date_fract.0,
            days_since_last_month: saved.days_since_last_month,
            tick_counter: saved.tick_counter.0,
            calendar_year: cache.calendar_year,
            calendar_month: cache.calendar_month,
            economy_year: cache.economy_year,
            economy_month: cache.economy_month,
        }
    }
}
