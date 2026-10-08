//! Replays independently executed upstream timer transitions and boundary order.
use ottd_core::{
    CalendarDate, ClockCache, ClockEvent, ClockSettings, ClockSnapshot, ClockState, DateFraction,
    EconomyDate, TickCounter, TimekeepingUnits,
};
use serde_json::Value;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn number<T: TryFrom<i128>>(value: &Value, field: &str) -> TestResult<T> {
    let number = value
        .get(field)
        .and_then(Value::as_u64)
        .map(i128::from)
        .or_else(|| value.get(field).and_then(Value::as_i64).map(i128::from))
        .ok_or_else(|| format!("missing integer {field}"))?;
    T::try_from(number).map_err(|_| format!("out-of-range integer {field}").into())
}
fn parse_state(value: &Value, settings: ClockSettings) -> TestResult<ClockState> {
    let snapshot = ClockSnapshot {
        date: CalendarDate::from_raw(number(value, "date")?)?,
        date_fract: DateFraction(number(value, "date_fract")?),
        calendar_sub_date_fract: number(value, "calendar_sub_date_fract")?,
        economy_date: EconomyDate(number(value, "economy_date")?),
        economy_date_fract: DateFraction(number(value, "economy_date_fract")?),
        days_since_last_month: number(value, "days_since_last_month")?,
        tick_counter: TickCounter(number(value, "tick_counter")?),
    };
    let cache = ClockCache {
        calendar_year: number(value, "calendar_year")?,
        calendar_month: number(value, "calendar_month")?,
        economy_year: number(value, "economy_year")?,
        economy_month: number(value, "economy_month")?,
    };
    Ok(ClockState::restore(snapshot, settings, cache)?)
}
#[test]
fn clock_dispatch_matches_native_before_after_vectors() -> TestResult<()> {
    let path = std::env::var_os("OTTD_GAMEPLAY_JSON").map_or_else(
        || {
            std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../reference/gameplay.json"
            ))
        },
        std::path::PathBuf::from,
    );
    let root: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let cases = root
        .get("clock_cases")
        .and_then(Value::as_array)
        .ok_or("missing clock_cases")?;
    assert!(cases.len() >= 15, "native boundary coverage missing");
    let mut steps = 0usize;
    for case in cases {
        let name = case
            .get("name")
            .and_then(Value::as_str)
            .ok_or("missing case name")?;
        let context = case.get("context").ok_or("missing context")?;
        let units = match number::<u8>(context, "timekeeping_units")? {
            0 => TimekeepingUnits::Calendar,
            1 => TimekeepingUnits::Wallclock,
            _ => return Err("invalid units".into()),
        };
        let settings = ClockSettings::new(units, number(context, "minutes_per_calendar_year")?)?;
        let paused = context
            .get("paused")
            .and_then(Value::as_bool)
            .ok_or("missing pause")?;
        let mut clock = parse_state(case.get("before").ok_or("missing before")?, settings)?;
        for (index, step) in case
            .get("steps")
            .and_then(Value::as_array)
            .ok_or("missing steps")?
            .iter()
            .enumerate()
        {
            let events = clock.advance(paused);
            let expected = parse_state(step.get("after").ok_or("missing after")?, settings)?;
            assert_eq!(clock, expected, "case {name} step {index}");
            assert_eq!(
                Some(events.calendar_progressed),
                step.get("calendar_progressed").and_then(Value::as_bool),
                "case {name} step {index}"
            );
            assert_eq!(
                serde_json::to_value(events.iter().map(ClockEvent::name).collect::<Vec<_>>())?,
                *step.get("events").ok_or("missing events")?,
                "case {name} step {index}"
            );
            let mut restored = ClockState::restore(clock.snapshot(), settings, clock.cache())?;
            let mut continued = clock;
            assert_eq!(restored.advance(paused), continued.advance(paused));
            assert_eq!(restored, continued);
            steps = steps.wrapping_add(1);
        }
    }
    println!(
        "matched {} native clock cases and {steps} steps, including resume and ordered events",
        cases.len()
    );
    Ok(())
}
