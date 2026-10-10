use super::{State, WorldTickError, unsupported};
use crate::world_access::{signed, unsigned};
use ottd_core::{
    CalendarDate, ClockSettings, ClockSnapshot, ClockState, DateFraction, EconomyDate, TickCounter,
    TimekeepingUnits,
};
use ottd_save::{WireValue, world::World};
pub(super) fn restore_clock(world: &World) -> Result<ClockState, WorldTickError> {
    let units = match unsigned(world, b"PATS", 0, "economy.timekeeping_units")? {
        0 => TimekeepingUnits::Calendar,
        1 => TimekeepingUnits::Wallclock,
        _ => return Err(unsupported("clock", "timekeeping units")),
    };
    let minutes = u16::try_from(unsigned(
        world,
        b"PATS",
        0,
        "economy.minutes_per_calendar_year",
    )?)
    .map_err(|_| unsupported("clock", "calendar speed"))?;
    let date = i32::try_from(signed(world, b"DATE", 0, "date")?)
        .map_err(|_| unsupported("clock", "date"))?;
    let narrow = |name| -> Result<u16, WorldTickError> {
        u16::try_from(unsigned(world, b"DATE", 0, name)?).map_err(|_| unsupported("clock", name))
    };
    Ok(ClockState::new(
        ClockSnapshot {
            date: CalendarDate::from_raw(date)
                .map_err(|_| unsupported("clock", "calendar date"))?,
            date_fract: DateFraction(narrow("date_fract")?),
            calendar_sub_date_fract: narrow("calendar_sub_date_fract")?,
            economy_date: EconomyDate(
                i32::try_from(signed(world, b"DATE", 0, "economy_date")?)
                    .map_err(|_| unsupported("clock", "economy date"))?,
            ),
            economy_date_fract: DateFraction(narrow("economy_date_fract")?),
            days_since_last_month: u32::try_from(unsigned(
                world,
                b"DATE",
                0,
                "days_since_last_month",
            )?)
            .map_err(|_| unsupported("clock", "day count"))?,
            tick_counter: TickCounter(unsigned(world, b"DATE", 0, "tick_counter")?),
        },
        ClockSettings::new(units, minutes)?,
    )?)
}
pub(super) fn save_clock(state: &mut State<'_>, s: ClockSnapshot) -> Result<(), WorldTickError> {
    for (name, value) in [
        ("date", i64::from(s.date.raw())),
        ("date_fract", i64::from(s.date_fract.0)),
        (
            "calendar_sub_date_fract",
            i64::from(s.calendar_sub_date_fract),
        ),
        ("economy_date", i64::from(s.economy_date.0)),
        ("economy_date_fract", i64::from(s.economy_date_fract.0)),
        ("days_since_last_month", i64::from(s.days_since_last_month)),
    ] {
        state.set_number(b"DATE", 0, name, value)?;
    }
    state.set(
        b"DATE",
        0,
        "tick_counter",
        WireValue::Unsigned(s.tick_counter.0),
    )
}
