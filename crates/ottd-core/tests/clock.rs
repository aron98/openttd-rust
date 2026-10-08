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

#[test]
fn rejects_invalid_economy_dates_and_caches() {
    let settings = ClockSettings::new(TimekeepingUnits::Wallclock, 24).unwrap();
    for raw in [-1, 1_800_000_360, i32::MAX] {
        let mut saved = snapshot().unwrap();
        saved.economy_date = EconomyDate(raw);
        assert!(ClockState::new(saved, settings).is_err());
    }
    let valid = ClockState::new(snapshot().unwrap(), settings).unwrap();
    let mut cache = valid.cache();
    cache.calendar_month = 12;
    assert!(ClockState::restore(valid.snapshot(), settings, cache).is_err());
    cache = valid.cache();
    cache.economy_year = 5_000_000;
    cache.economy_month = 0;
    assert!(ClockState::restore(valid.snapshot(), settings, cache).is_err());
    let mut saved = snapshot().unwrap();
    saved.economy_date_fract = DateFraction(74);
    assert!(ClockState::new(saved, settings).is_err());
}

proptest::proptest! {
    #[test]
    fn restore_preserves_continuation(
        date in 0i32..1_826_212_866,
        economy in 0i32..1_800_000_360,
        fraction in 0u16..74,
        subfraction in proptest::prelude::any::<u16>(),
        minutes in 12u16..=10080,
        ticks in proptest::prelude::any::<u64>(),
    ) {
        let settings = ClockSettings::new(TimekeepingUnits::Wallclock,minutes).unwrap();
        let saved = ClockSnapshot {
            date: CalendarDate::from_raw(date).unwrap(),date_fract:DateFraction(fraction),calendar_sub_date_fract:subfraction,
            economy_date:EconomyDate(economy),economy_date_fract:DateFraction(fraction),days_since_last_month:0,tick_counter:TickCounter(ticks),
        };
        let mut state = ClockState::new(saved,settings).unwrap();
        for _ in 0..75 {
            state.advance(false);
            let restored = ClockState::restore(state.snapshot(),settings,state.cache()).unwrap();
            proptest::prop_assert_eq!(state,restored);
            proptest::prop_assert!(state.snapshot().date_fract.0 <74);
            proptest::prop_assert!(state.snapshot().economy_date_fract.0 <74);
        }
    }
}
