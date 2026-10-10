//! Fast transaction checks using a real original-command-generated native29 save.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{WorldTickError, runtime::SimulationRuntime};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn moving() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/road-movement/bus-first.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

fn other_side() -> Result<SimulationRuntime> {
    let mut world = moving()?;
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"PATS",
        record: 0,
        path: vec![PathElement::Field("vehicle.road_side".into())],
        value: WireValue::Unsigned(0),
    }])?;
    Ok(SimulationRuntime::restore_vanilla(world)?)
}

#[test]
fn zero_calls_preserve_unsupported_side_and_all_supported_runtime() -> Result {
    let mut runtime = other_side()?;
    let before = runtime.saved_json()?;
    let caches = runtime.road_caches().clone();
    let spatial = runtime.single_road_tile_occupancy()?;
    assert!(runtime.advance_world(0)?.ticks.is_empty());
    assert_eq!(runtime.saved_json()?, before);
    assert_eq!(runtime.road_caches(), &caches);
    assert_eq!(runtime.single_road_tile_occupancy()?, spatial);
    Ok(())
}

#[test]
fn unsupported_side_is_typed_and_does_not_publish() -> Result {
    let mut runtime = other_side()?;
    let before = runtime.saved_json()?;
    let caches = runtime.road_caches().clone();
    let spatial = runtime.single_road_tile_occupancy()?;
    assert!(matches!(
        runtime.advance_world(1),
        Err(WorldTickError::Unsupported { phase: "road", .. })
    ));
    assert_eq!(runtime.saved_json()?, before);
    assert_eq!(runtime.road_caches(), &caches);
    assert_eq!(runtime.single_road_tile_occupancy()?, spatial);
    Ok(())
}

#[test]
fn offset_month_refuses_after_provisional_rng_and_motion() -> Result {
    let mut world = moving()?;
    let date = ottd_core::CalendarDate::from_ymd(2100, 0, 31)?.raw();
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"DATE",
        record: 0,
        path: vec![PathElement::Field("economy_date".into())],
        value: WireValue::Signed(i64::from(date)),
    }])?;
    let mut prefix = SimulationRuntime::restore_vanilla(world.clone())?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.saved_json()?;
    let caches = runtime.road_caches().clone();
    let spatial = runtime.single_road_tile_occupancy()?;
    prefix.advance_world(37)?;
    let provisional = prefix.saved_json()?;
    assert_ne!(
        provisional.pointer("/chunks/DATE/records/0/random_state[0]"),
        before.pointer("/chunks/DATE/records/0/random_state[0]")
    );
    assert_ne!(
        provisional.pointer("/chunks/VEHS"),
        before.pointer("/chunks/VEHS")
    );
    assert!(matches!(
        runtime.advance_world(74),
        Err(WorldTickError::Unsupported {
            phase: "road_economy",
            ..
        })
    ));
    assert_eq!(runtime.saved_json()?, before);
    assert_eq!(runtime.road_caches(), &caches);
    assert_eq!(runtime.single_road_tile_occupancy()?, spatial);
    Ok(())
}

#[test]
fn phased_clock_exposes_calendar_before_economy_before_tick() -> Result {
    use ottd_core::{
        CalendarDate, ClockPhase, ClockSettings, ClockSnapshot, ClockState, DateFraction,
        EconomyDate, TickCounter, TimekeepingUnits,
    };
    // Given aligned clocks immediately before a month boundary.
    let date = CalendarDate::from_ymd(2100, 0, 31)?;
    let initial = ClockSnapshot {
        date,
        date_fract: DateFraction(73),
        economy_date: EconomyDate(date.raw()),
        economy_date_fract: DateFraction(73),
        calendar_sub_date_fract: 0,
        days_since_last_month: 30,
        tick_counter: TickCounter(99),
    };
    let mut clock = ClockState::new(initial, ClockSettings::new(TimekeepingUnits::Calendar, 12)?)?;
    let mut phases = Vec::new();
    // When the canonical clock executes one normal-game call.
    clock.try_advance(false, |phase, state, _| {
        phases.push((phase, state.snapshot()));
        Ok::<(), std::convert::Infallible>(())
    })?;
    // Then consumers see native intermediate timer state, not a fully advanced duplicate loop.
    assert_eq!(
        phases.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        vec![ClockPhase::Calendar, ClockPhase::Economy, ClockPhase::Tick]
    );
    let calendar = phases.first().ok_or("calendar")?.1;
    let economy = phases.get(1).ok_or("economy")?.1;
    assert_eq!(calendar.date.raw(), date.raw().wrapping_add(1));
    assert_eq!(calendar.economy_date, initial.economy_date);
    assert_eq!(economy.economy_date.0, calendar.date.raw());
    assert_eq!(economy.tick_counter, initial.tick_counter);
    assert_eq!(clock.snapshot().tick_counter.0, 100);
    Ok(())
}
