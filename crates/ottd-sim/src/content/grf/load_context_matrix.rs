use super::{Result, grf_control_cases as fixture};
use crate::content::{
    Climate,
    grf::{Palette, load_context::EnvironmentInput},
};

pub(super) struct Case {
    pub program: fixture::Case,
    pub environment: EnvironmentInput,
    pub palette: Palette,
}

fn reads() -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    for number in 0x80_u8..=0xfe {
        records.push(fixture::set(number.wrapping_sub(0x80), 0, number, 0, 0));
    }
    records
}

fn case(name: &str, records: &[Vec<u8>]) -> Result<Case> {
    let mut settings = super::settings()?;
    settings.map = ottd_core::MapDimensions::new(64, 64)?;
    Ok(Case {
        program: fixture::single(name, records, &[], 1)?,
        environment: EnvironmentInput {
            saved: super::saved()?,
            settings,
        },
        palette: Palette::Windows,
    })
}

pub(super) fn all() -> Result<Vec<Case>> {
    let reads = reads();
    let mut cases = Vec::new();
    for (name, climate) in [
        ("temperate", Climate::Temperate),
        ("arctic", Climate::Arctic),
        ("tropic", Climate::Tropic),
        ("toyland", Climate::Toyland),
    ] {
        let mut entry = case(&format!("globals-{name}"), &reads)?;
        entry.environment.settings.climate = climate;
        cases.push(entry);
    }
    for year in [0, 1919, 1920, 2000, 2090, 2091, 5_000_000] {
        let mut entry = case(&format!("calendar-{year}"), &reads)?;
        entry.environment.saved.date = ottd_core::CalendarDate::from_ymd(year, 0, 1)?;
        if year == 5_000_000 {
            entry.environment.settings.starting_year = year;
            entry.environment.saved.economy_date =
                ottd_core::EconomyDate(ottd_core::CalendarDate::from_ymd(year, 11, 31)?.raw());
        }
        cases.push(entry);
    }
    for units in [
        ottd_core::TimekeepingUnits::Calendar,
        ottd_core::TimekeepingUnits::Wallclock,
    ] {
        let mut entry = case(&format!("network-{units:?}"), &reads)?;
        entry.program.networking = true;
        entry.environment.settings.timekeeping_units = units;
        if units == ottd_core::TimekeepingUnits::Wallclock {
            entry.environment.settings.starting_year = 5_000_000;
            entry.environment.saved.economy_date = ottd_core::EconomyDate(1_800_000_359);
        }
        cases.push(entry);
    }
    for mode in 0..=3 {
        let mut entry = case(&format!("mode-{mode}"), &reads)?;
        entry.environment.settings.game_mode = mode;
        cases.push(entry);
    }
    let mut palette = case("palette-dos", &reads)?;
    palette.palette = Palette::Dos;
    cases.push(palette);
    let mut patch_records = Vec::new();
    for variable in 0..=127 {
        patch_records.push(fixture::set(variable, 0, variable, 0xfe, 0xffff));
    }
    cases.push(case("patch-variables-low", &patch_records)?);
    patch_records.clear();
    for variable in 128_u8..=255 {
        patch_records.push(fixture::set(
            variable.wrapping_sub(128),
            0,
            variable,
            0xfe,
            0xffff,
        ));
    }
    cases.push(case("patch-variables-high", &patch_records)?);
    for start in [0_u8, 64, 128, 192] {
        let mut records = Vec::new();
        for index in 0..64 {
            records.push(fixture::set(index, 0, 0xff, 0, 0));
            records.push(fixture::condition(
                9,
                0x85,
                1,
                1,
                u32::from(start.wrapping_add(index)),
                0,
                1,
            ));
            records.push(fixture::set(index, 0, 0xff, 0, 1));
        }
        cases.push(case(&format!("patch-bits-{start}"), &records)?);
    }
    for disabled in [false, true] {
        for value in [0, 0xff03_0201, u32::MAX, 0x8000_0000] {
            let mut records = Vec::new();
            for (index, target) in [0x8e, 0x8f, 0x9e].into_iter().enumerate() {
                records.push(fixture::set(target, 0, 0xff, 0, value));
                records.push(fixture::set(u8::try_from(index)?, 0, target, 0, 0));
            }
            let mut entry = case(&format!("targets-{disabled}-{value:08x}"), &records)?;
            entry.environment.settings.disable_elrails = disabled;
            cases.push(entry);
        }
    }
    settings_cases(&mut cases)?;
    registry_cases(&mut cases)?;
    for (width, height) in [(128, 256), (256, 128)] {
        let mut entry = case(
            &format!("actual-map-{width}-{height}"),
            &[fixture::set(0, 0, 0x13, 0xfe, 0xffff)],
        )?;
        entry.environment.settings.map = ottd_core::MapDimensions::new(width, height)?;
        cases.push(entry);
    }
    Ok(cases)
}

fn registry_cases(cases: &mut Vec<Case>) -> Result {
    for is_static in [false, true] {
        let mut entry = case(&format!("cross-file-targets-static-{is_static}"), &[])?;
        let first = fixture::source(
            0x4141_4141,
            &[
                fixture::set(0x8e, 0, 0xff, 0, u32::MAX),
                fixture::set(0x9e, 0, 0xff, 0, 0xff),
                fixture::set(0x8f, 0, 0xff, 0, 0x0003_0201),
                fixture::set(0, 0, 0x8e, 0, 0),
                fixture::set(1, 0, 0x9e, 0, 0),
            ],
            &[],
            1,
        )?;
        let mut second = fixture::source(
            0x4242_4242,
            &[
                fixture::set(0, 0, 0x8e, 0, 0),
                fixture::set(1, 0, 0x9e, 0, 0),
                fixture::set(0x9e, 0, 0xff, 0, 8),
                fixture::set(2, 0, 0x9e, 0, 0),
            ],
            &[],
            1,
        )?;
        second.flags.is_static = is_static;
        let third = fixture::source(
            0x4343_4343,
            &[
                fixture::set(0, 0, 0x8e, 0, 0),
                fixture::set(1, 0, 0x9e, 0, 0),
                fixture::set(2, 0, 0x8f, 0, 0),
            ],
            &[],
            1,
        )?;
        entry.program.sources = vec![first, second, third];
        cases.push(entry);
    }
    let records = (0x80..=0xff)
        .filter(|target| !matches!(target, 0x8e | 0x8f | 0x9e))
        .map(|target| fixture::set(target, 0, 0xff, 0, u32::MAX))
        .chain(reads())
        .collect::<Vec<_>>();
    cases.push(case("ignored-special-targets", &records)?);
    Ok(())
}

type SettingChange = (
    &'static str,
    fn(&mut crate::content::grf::load_context::EnvironmentSettings),
);

fn settings_cases(cases: &mut Vec<Case>) -> Result {
    let changes: &[SettingChange] = &[
        ("airports", |s| s.patch.never_expire_airports = true),
        ("bridge16", |s| s.patch.max_bridge_length = 16),
        ("bridge17", |s| s.patch.max_bridge_length = 17),
        ("vehicles", |s| s.patch.never_expire_vehicles = true),
        ("noise", |s| s.patch.station_noise_level = true),
        ("gradual", |s| s.patch.gradual_loading = false),
        ("signal0", |s| s.patch.train_signal_side = 0),
        ("signal2", |s| s.patch.train_signal_side = 2),
        ("elrails", |s| s.disable_elrails = true),
        ("slopes", |s| s.patch.build_on_slopes = false),
        ("freight1", |s| s.freight_trains = 1),
        ("wagon", |s| s.patch.wagon_speed_limits = false),
        ("town-roads", |s| s.patch.allow_town_roads = false),
        ("generating", |s| {
            s.patch.allow_town_roads = false;
            s.patch.generating_world = true;
        }),
        ("improved", |s| s.patch.improved_load = false),
        ("engines", |s| s.patch.dynamic_engines = false),
        ("inflation", |s| s.patch.inflation = false),
    ];
    for (name, change) in changes {
        let mut entry = case(&format!("flag-setting-{name}"), &[])?;
        entry.program.sources.clear();
        for (index, start) in [0_u8, 64, 128, 192].into_iter().enumerate() {
            let mut records = Vec::new();
            for bit in 0..64 {
                records.push(fixture::set(bit, 0, 0xff, 0, 0));
                records.push(fixture::condition(
                    9,
                    0x85,
                    1,
                    1,
                    u32::from(start.wrapping_add(bit)),
                    0,
                    1,
                ));
                records.push(fixture::set(bit, 0, 0xff, 0, 1));
            }
            entry.program.sources.push(fixture::source(
                0x4141_4141_u32
                    .checked_add(u32::try_from(index)?)
                    .ok_or("id")?,
                &records,
                &[],
                1,
            )?);
        }
        change(&mut entry.environment.settings);
        cases.push(entry);
    }
    for plane_speed in [1, 3, 4, 255] {
        let mut entry = case(
            &format!("plane-speed-{plane_speed}"),
            &[fixture::set(0, 0, 0x10, 0xfe, 0xffff)],
        )?;
        entry.environment.settings.plane_speed = plane_speed;
        cases.push(entry);
    }
    for (height, snowline) in [(19, 20), (20, 20), (255, 254), (255, 255)] {
        for version in [7, 8] {
            let mut entry = case(&format!("snow-{version}-{height}-{snowline}"), &[])?;
            let mut info = fixture::info(0x4141_4141);
            *info.get_mut(1).ok_or("version")? = version;
            entry.program.sources.first_mut().ok_or("source")?.bytes =
                Some(fixture::encode(&[info, fixture::set(0, 0, 0xa0, 0, 0)], 1)?);
            entry.environment.settings.height_limit = height;
            entry.environment.settings.snowline = snowline;
            cases.push(entry);
        }
    }
    Ok(())
}

#[test]
fn context_matrix_executes_all_generated_programs() -> Result {
    let cases = all()?;
    for case in &cases {
        let inputs = case
            .program
            .sources
            .iter()
            .map(|source| {
                let mut input = source.input();
                input.palette = case.palette;
                input
            })
            .collect::<Vec<_>>();
        let (_, environment) = crate::content::grf::load::run_with_environment(
            &inputs,
            &[],
            crate::content::grf::ControlOptions {
                networking: case.program.networking,
                ..Default::default()
            },
            Some(case.environment),
        )
        .map_err(|error| format!("{}: {error}", case.program.name))?;
        assert_eq!(
            environment.ok_or("context")?.0.current,
            case.environment.saved
        );
    }
    println!("executed {} context programs", cases.len());
    Ok(())
}
