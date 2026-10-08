//! Checked domain boundaries and deterministic state restoration.
use ottd_core::{CalendarDate, MapDimensions, Randomizer};

#[test]
fn rng_restores_saved_state_and_wraps() {
    let mut rng = Randomizer::from_state([0, 0]);
    assert_eq!(rng.next_u32(), u32::MAX);
    let state = rng.state();
    assert_eq!(rng.next_u32(), Randomizer::from_state(state).next_u32());
}

#[test]
fn coordinates_enforce_map_bounds() {
    let map = MapDimensions::new(4096, 64).unwrap();
    let tile = map.tile(4095, 63).unwrap();
    assert_eq!(tile.raw(), 262_143);
    assert_eq!(map.coordinates(tile), Some((4095, 63)));
    assert!(map.tile(4096, 0).is_none());
    assert!(map.index(262_144).is_none());
    for invalid in [0, 32, 63, 65, 4097, u32::MAX] {
        assert!(MapDimensions::new(invalid, 64).is_err());
    }
}

#[test]
fn calendar_checks_leap_days_and_arithmetic_limits() {
    let leap = CalendarDate::from_ymd(0, 1, 29).unwrap();
    assert_eq!(leap.raw(), 59);
    assert_eq!(leap.checked_add_days(1).unwrap().ymd(), (0, 2, 1));
    assert!(CalendarDate::from_ymd(1900, 1, 29).is_err());
    assert!(CalendarDate::from_ymd(2000, 1, 29).is_ok());
    assert!(CalendarDate::from_ymd(0, 0, 0).is_err());
    assert!(CalendarDate::from_ymd(0, 12, 1).is_err());
    assert!(CalendarDate::from_ymd(5_000_001, 0, 1).is_err());
    assert!(
        CalendarDate::from_raw(0)
            .unwrap()
            .checked_add_days(-1)
            .is_none()
    );
    let max = CalendarDate::from_ymd(5_000_000, 11, 31).unwrap();
    assert!(max.checked_add_days(1).is_none());
    assert!(max.checked_add_days(i32::MAX).is_none());
    assert_eq!(max.ymd(), (5_000_000, 11, 31));
}

proptest::proptest! {
    #[test]
    fn dates_roundtrip(raw in 0..=1_826_212_865i32) {
        let date = CalendarDate::from_raw(raw).unwrap();
        let (year, month, day) = date.ymd();
        proptest::prop_assert_eq!(CalendarDate::from_ymd(year, month, day).unwrap(), date);
    }
}
