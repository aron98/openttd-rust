//! Test-only fixtures for the private loading environment.
use super::{
    ControlOptions, GrfIdentity, LoadFlags, LoadInput, Palette,
    load_context::{Environment, EnvironmentInput, EnvironmentSettings, FileGlobals},
};
#[path = "../../../tests/grf_control_cases/mod.rs"]
pub mod grf_control_cases;
#[path = "load_context_matrix.rs"]
mod matrix;
#[path = "load_context_native.rs"]
pub mod native;
use crate::content::Climate;
use ottd_core::{
    CalendarDate, ClockSnapshot, DateFraction, EconomyDate, MapDimensions, TickCounter,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn saved() -> Result<ClockSnapshot> {
    Ok(ClockSnapshot {
        date: CalendarDate::from_ymd(2000, 1, 29)?,
        date_fract: DateFraction(37),
        calendar_sub_date_fract: 9,
        economy_date: EconomyDate(12345),
        economy_date_fract: DateFraction(17),
        days_since_last_month: 7,
        tick_counter: TickCounter(65537),
    })
}
fn settings() -> Result<EnvironmentSettings> {
    Ok(EnvironmentSettings {
        display_options: 0xa5,
        timekeeping_units: ottd_core::TimekeepingUnits::Calendar,
        patch: super::load_patch::PatchSettings {
            never_expire_airports: false,
            max_bridge_length: 64,
            never_expire_vehicles: false,
            station_noise_level: false,
            gradual_loading: true,
            train_signal_side: 1,
            build_on_slopes: true,
            wagon_speed_limits: true,
            allow_town_roads: true,
            generating_world: false,
            improved_load: true,
            dynamic_engines: true,
            inflation: true,
        },
        game_mode: 1,
        starting_year: 1950,
        climate: Climate::Arctic,
        right_hand_traffic: true,
        disable_elrails: false,
        map: MapDimensions::new(128, 256)?,
        height_limit: 255,
        snowline: 20,
        generation_seed: 12345,
        freight_trains: 3,
        plane_speed: 2,
    })
}

#[test]
fn network_wallclock_economy_uses_360_day_start_year() -> Result {
    let mut settings = settings()?;
    settings.timekeeping_units = ottd_core::TimekeepingUnits::Wallclock;
    let environment = Environment::new(saved()?, settings, true)?;
    assert_eq!(environment.current.economy_date, EconomyDate(702_000));
    Ok(())
}

#[test]
fn network_load_uses_start_date_without_mutating_saved_clock() -> Result {
    let saved = saved()?;
    let environment = Environment::new(saved, settings()?, true)?;
    let file = FileGlobals::default();
    assert_eq!(
        environment.global(0x23, file, 8, Palette::Windows)?,
        Some(u32::try_from(CalendarDate::from_ymd(1950, 0, 1)?.raw())?)
    );
    assert_eq!(
        environment.global(0x09, file, 8, Palette::Windows)?,
        Some(0)
    );
    assert_eq!(
        environment.global(0x0a, file, 8, Palette::Windows)?,
        Some(0)
    );
    assert_eq!(environment.saved, saved);
    assert_eq!(environment.current_display, 0);
    assert_eq!(environment.saved_display, 0xa5);
    Ok(())
}

#[test]
fn calendar_context_preserves_leap_day_and_tick_low_word() -> Result {
    let environment = Environment::new(saved()?, settings()?, false)?;
    let file = FileGlobals::default();
    assert_eq!(
        environment.global(0x02, file, 8, Palette::Windows)?,
        Some(1 | (28 << 8) | (1 << 15) | (59 << 16))
    );
    assert_eq!(
        environment.global(0x09, file, 8, Palette::Windows)?,
        Some(37 * 885)
    );
    assert_eq!(
        environment.global(0x0a, file, 8, Palette::Windows)?,
        Some(1)
    );
    Ok(())
}

#[test]
fn static_misc_write_changes_only_safe_global_bit_and_own_width() -> Result {
    let mut environment = Environment::new(saved()?, settings()?, false)?;
    let mut caller = FileGlobals::default();
    environment.target(0x9e, 0x12, false, &mut caller);
    let mut static_file = FileGlobals::default();
    environment.target(0x9e, 0xff, true, &mut static_file);
    assert_eq!(
        environment.global(0x1e, caller, 8, Palette::Windows)?,
        Some(0x52)
    );
    assert_eq!(
        environment.global(0x1e, static_file, 8, Palette::Windows)?,
        Some(0x5a)
    );
    Ok(())
}

#[test]
fn rail_cost_target_respects_disabled_electric_rail_mapping() -> Result {
    let mut settings = settings()?;
    settings.disable_elrails = true;
    let mut environment = Environment::new(saved()?, settings, false)?;
    let mut file = FileGlobals::default();
    environment.target(0x8f, 0xff03_0201, false, &mut file);
    assert_eq!(environment.rail_costs, [1, 1, 2, 3]);
    assert_eq!(
        environment.global(0x0f, file, 8, Palette::Windows)?,
        Some(0x0003_0201)
    );
    Ok(())
}

#[test]
fn date_global_does_not_require_a_matching_config_identity() -> Result {
    let source = grf_control_cases::source(
        0x4141_4141,
        &[grf_control_cases::set(0, 0, 0x81, 0, 0)],
        &[],
        1,
    )?;
    let mut input = source.input();
    input.identity.grfid = 0x4242_4242;
    input.flags.init_only = true;
    let (report, environment) = super::load::run_with_environment(
        &[input],
        &[],
        ControlOptions::default(),
        Some(EnvironmentInput {
            saved: saved()?,
            settings: settings()?,
        }),
    )?;
    assert_eq!(
        report.files.first().ok_or("file")?.parameters.as_deref(),
        Some([80].as_slice())
    );
    assert_eq!(environment.ok_or("environment")?.0.current, saved()?);
    Ok(())
}

#[test]
fn existing_control_programs_remain_admitted_with_explicit_context() -> Result {
    for case in grf_control_cases::all()? {
        let inputs = case
            .sources
            .iter()
            .map(grf_control_cases::Source::input)
            .collect::<Vec<_>>();
        let (_, environment) = super::load::run_with_environment(
            &inputs,
            &[],
            ControlOptions {
                networking: case.networking,
                ..ControlOptions::default()
            },
            Some(EnvironmentInput {
                saved: saved()?,
                settings: settings()?,
            }),
        )
        .map_err(|error| format!("{}: {error}", case.name))?;
        assert_eq!(environment.ok_or("environment")?.0.current, saved()?);
    }
    Ok(())
}

#[test]
fn empty_context_load_charges_source_budget() -> Result {
    let error = super::load::run_with_environment(
        &[],
        &[],
        ControlOptions {
            max_source_bytes: 0,
            ..Default::default()
        },
        Some(EnvironmentInput {
            saved: saved()?,
            settings: settings()?,
        }),
    )
    .err()
    .ok_or("zero budget unexpectedly admitted environment")?;
    assert!(matches!(
        error,
        super::ControlLoadError::ResourceLimit {
            resource: "source/config bytes",
            ..
        }
    ));
    Ok(())
}

#[test]
fn final_context_payload_obeys_exact_host_boundary() -> Result {
    let input = EnvironmentInput {
        saved: saved()?,
        settings: settings()?,
    };
    let (baseline, _) =
        super::load::run_with_environment(&[], &[], ControlOptions::default(), None)?;
    let boundary = baseline
        .events
        .len()
        .checked_mul(std::mem::size_of::<super::LoadEvent>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<super::load_context::EnvironmentReport>())
        })
        .ok_or("budget overflow")?;
    let error = super::load::run_with_environment(
        &[],
        &[],
        ControlOptions {
            max_trace_bytes: boundary.checked_sub(1).ok_or("budget")?,
            ..Default::default()
        },
        Some(input),
    )
    .err()
    .ok_or("short trace budget admitted context")?;
    assert!(matches!(
        error,
        super::ControlLoadError::ResourceLimit {
            resource: "trace payload bytes",
            ..
        }
    ));
    let (_, restored) = super::load::run_with_environment(
        &[],
        &[],
        ControlOptions {
            max_trace_bytes: boundary,
            max_source_bytes: std::mem::size_of::<EnvironmentInput>(),
            ..Default::default()
        },
        Some(input),
    )?;
    assert_eq!(restored.ok_or("context")?.0.current, input.saved);
    Ok(())
}

#[test]
fn offline_context_refuses_out_of_range_starting_year() -> Result {
    for year in [-1, 5_000_001] {
        let mut settings = settings()?;
        settings.starting_year = year;
        assert!(
            matches!(Environment::new(saved()?, settings, false),Err(super::load_context::ContextError::StartingYear(actual)) if actual==year),
            "admitted starting year {year}"
        );
    }
    Ok(())
}

#[test]
fn context_refuses_out_of_range_saved_economy_date_before_normalization() -> Result {
    for units in [
        ottd_core::TimekeepingUnits::Calendar,
        ottd_core::TimekeepingUnits::Wallclock,
    ] {
        for networking in [false, true] {
            for raw in [-1, i32::MAX] {
                let mut settings = settings()?;
                settings.timekeeping_units = units;
                let mut saved = saved()?;
                saved.economy_date = EconomyDate(raw);
                assert!(
                    matches!(Environment::new(saved, settings, networking),Err(super::load_context::ContextError::EconomyDate {date,units:actual}) if date==raw && actual==units),
                    "admitted economy date {raw} in {units:?}, network {networking}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn context_admits_both_economy_upper_boundaries() -> Result {
    for (units, raw) in [
        (
            ottd_core::TimekeepingUnits::Calendar,
            CalendarDate::from_ymd(5_000_000, 11, 31)?.raw(),
        ),
        (ottd_core::TimekeepingUnits::Wallclock, 1_800_000_359),
    ] {
        let mut settings = settings()?;
        settings.starting_year = 5_000_000;
        settings.timekeeping_units = units;
        let mut saved = saved()?;
        saved.economy_date = EconomyDate(raw);
        assert_eq!(Environment::new(saved, settings, false)?.current, saved);
        saved.economy_date = EconomyDate(raw.checked_add(1).ok_or("date overflow")?);
        assert!(matches!(
            Environment::new(saved, settings, false),
            Err(super::load_context::ContextError::EconomyDate { .. })
        ));
    }
    Ok(())
}
