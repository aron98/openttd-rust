//! Clock transitions and validation.

use ottd_core::{
    CalendarDate, ClockSettings, ClockSnapshot, ClockState, DateFraction, EconomyDate, TickCounter,
    TimekeepingUnits,
};

fn snapshot() -> Result<ClockSnapshot, ottd_core::InvalidDate> {
    Ok(ClockSnapshot {
        date: CalendarDate::from_ymd(2000, 1, 28)?,
        date_fract: DateFraction(73),
        calendar_sub_date_fract: 0,
        economy_date: EconomyDate(CalendarDate::from_ymd(2000, 1, 28)?.raw()),
        economy_date_fract: DateFraction(73),
        days_since_last_month: 27,
        tick_counter: TickCounter(u64::MAX),
    })
}

#[test]
fn leap_day_and_tick_wrap() {
    let mut clock = ClockState::new(
        snapshot().unwrap(),
        ClockSettings::new(TimekeepingUnits::Calendar, 12).unwrap(),
    )
    .unwrap();
    let events = clock.advance(false);
    assert_eq!(clock.snapshot().date.ymd(), (2000, 1, 29));
    assert_eq!(clock.snapshot().date_fract, DateFraction(0));
    assert_eq!(clock.snapshot().tick_counter, TickCounter(0));
    assert!(events.calendar_progressed);
    assert_eq!(
        events
            .iter()
            .map(ottd_core::ClockEvent::name)
            .collect::<Vec<_>>(),
        ["calendar_day", "economy_day", "economy_week"]
    );
}

#[test]
fn pause_is_exact_noop() {
    let mut clock = ClockState::new(
        snapshot().unwrap(),
        ClockSettings::new(TimekeepingUnits::Calendar, 12).unwrap(),
    )
    .unwrap();
    let before = clock;
    let events = clock.advance(true);
    assert_eq!(clock, before);
    assert!(!events.calendar_progressed);
    assert_eq!(events.iter().count(), 0);
}

#[test]
fn rejects_unattainable_settings_and_invalid_fractions() {
    for minutes in [0, 1, 11, 13, 10080] {
        assert!(ClockSettings::new(TimekeepingUnits::Calendar, minutes).is_err());
    }
    for minutes in [1, 11, 10081, u16::MAX] {
        assert!(ClockSettings::new(TimekeepingUnits::Wallclock, minutes).is_err());
    }
    let mut input = snapshot().unwrap();
    input.date_fract = DateFraction(74);
    assert!(
        ClockState::new(
            input,
            ClockSettings::new(TimekeepingUnits::Calendar, 12).unwrap()
        )
        .is_err()
    );
}
