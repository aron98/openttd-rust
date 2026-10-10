use super::{
    ControlLoadError, ControlOptions, LoadStatus,
    load::{RuntimeInputs, run_with_currency},
    load_context_tests::grf_control_cases as programs,
    load_currency::CurrencyOwner,
    load_language_state::{LanguageInput, LanguageLimits, LanguageReport},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[path = "load_cargo_baseline_tests.rs"]
mod cargo_baseline;

#[path = "load_cargo_native_tests.rs"]
mod cargo_native;

#[path = "load_cargo_units.rs"]
mod cargo_units;

#[path = "load_engine_baseline_tests.rs"]
mod engine_baseline;

#[path = "load_engine_units.rs"]
mod engine_units;

#[path = "load_engine_ci_native.rs"]
mod engine_ci;

#[path = "load_currency_properties_tests.rs"]
mod properties;

#[path = "load_currency_property_units.rs"]
mod property_units;

fn run(
    sources: &[programs::Source],
    custom: Option<&CurrencyOwner>,
    options: ControlOptions,
) -> Result<(super::ControlLoadReport, LanguageReport)> {
    let mut pack = vec![0; 572];
    pack.get_mut(..4)
        .ok_or("magic")?
        .copy_from_slice(&0x474e_414c_u32.to_le_bytes());
    pack.get_mut(4..8)
        .ok_or("version")?
        .copy_from_slice(&0x2ad1_09ab_u32.to_le_bytes());
    let packs = [pack.as_slice()];
    let inputs = sources
        .iter()
        .map(programs::Source::input)
        .collect::<Vec<_>>();
    let (report, _, language) = run_with_currency(
        &inputs,
        &[],
        options,
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected: 0,
                limits: LanguageLimits::default(),
            }),
        },
        custom,
    )?;
    Ok((report, language.ok_or("language report")?))
}

fn property(index: u16, string: u16) -> Vec<u8> {
    let mut bytes = vec![0, 8, 1, 1, 255];
    bytes.extend(index.to_le_bytes());
    bytes.push(0x0a);
    bytes.extend(string.to_le_bytes());
    bytes
}

#[test]
fn currency_session_finalizes_after_later_string_definition() -> Result {
    let source = programs::source(
        7,
        &[property(256, 0xd800), vec![4, 0, 0x81, 1, 0, 0xd8, b'A', 0]],
        &[],
        1,
    )?;
    let (_, language) = run(&[source], None, ControlOptions::default())?;
    let state = language.currency.ok_or("currency")?;
    assert_eq!(state.owners.entries.first().ok_or("owner")?.name, 0x20000);
    assert!(state.pending.is_empty());
    let queued = language
        .events
        .iter()
        .find_map(|event| event.currency.as_ref())
        .ok_or("queued event")?;
    assert_eq!(queued.owners.entries.first().ok_or("owner")?.name, 2);
    assert_eq!(queued.pending.len(), 1);
    assert!(
        language
            .events
            .iter()
            .filter(|event| event.stage == 4)
            .all(|event| event.currency.is_none())
    );
    Ok(())
}

#[test]
fn disabled_currency_definition_retains_queued_assignment() -> Result {
    let source = programs::source(
        7,
        &[
            property(0, 0xd800),
            vec![4, 0, 0x81, 1, 0, 0xd8, b'A', 0],
            vec![0x13, 8, 0, 0, 0],
        ],
        &[],
        1,
    )?;
    let target = programs::source(8, &[], &[], 1)?;
    let (report, language) = run(&[source, target], None, ControlOptions::default())?;
    assert_eq!(
        report.files.first().ok_or("file")?.status,
        LoadStatus::Disabled
    );
    assert_eq!(
        language
            .currency
            .ok_or("currency")?
            .owners
            .entries
            .first()
            .ok_or("owner")?
            .name,
        0x20000
    );
    Ok(())
}

#[test]
fn second_currency_load_preserves_custom_but_rebuilds_strings() -> Result {
    let source = programs::source(
        7,
        &[property(31, 0xd800), vec![4, 0, 0x81, 1, 0, 0xd8, b'A', 0]],
        &[],
        1,
    )?;
    let (_, first) = run(&[source], None, ControlOptions::default())?;
    let first = first.currency.ok_or("first currency")?;
    let (_, next) = run(
        &[],
        Some(first.owners.entries.get(31).ok_or("owner")?),
        ControlOptions::default(),
    )?;
    assert!(next.strings.is_empty());
    let next = next.currency.ok_or("next currency")?;
    assert_eq!(
        next.owners.entries.get(31).ok_or("owner")?,
        first.owners.entries.get(31).ok_or("owner")?
    );
    assert!(next.pending.is_empty());
    Ok(())
}

#[test]
fn currency_repeated_assignments_obey_cumulative_trace_budget() -> Result {
    let options = ControlOptions {
        max_trace_bytes: 128 * 1024,
        ..ControlOptions::default()
    };
    let single = programs::source(7, &[property(0, 0xd800)], &[], 1)?;
    run(&[single], None, options)?;
    let repeated = programs::source(7, &vec![property(0, 0xd800); 100], &[], 1)?;
    let error = run(&[repeated], None, options).expect_err("repeated work exceeds host budget");
    assert!(matches!(
        error.downcast_ref::<ControlLoadError>(),
        Some(ControlLoadError::ResourceLimit { .. })
    ));
    Ok(())
}

#[test]
fn currency_adjacent_properties_remain_unsupported() -> Result {
    let source = programs::source(7, &[vec![0, 8, 1, 1, 0, 0x10, 0, 0, 0, 0]], &[], 1)?;
    let error = run(&[source], None, ControlOptions::default())
        .expect_err("snowline property not implemented");
    assert!(matches!(
        error.downcast_ref::<ControlLoadError>(),
        Some(ControlLoadError::Unsupported { action: 0, .. })
    ));
    Ok(())
}

#[test]
fn currency_count_wrap_invalid_and_empty_items_follow_reader_order() -> Result {
    let wrap = programs::source(
        7,
        &[vec![0, 8, 1, 2, 255, 255, 255, 0x0a, 0, 0, 0, 0]],
        &[],
        1,
    )?;
    let (_, report) = run(&[wrap], None, ControlOptions::default())?;
    assert_eq!(
        report
            .currency
            .ok_or("currency")?
            .owners
            .entries
            .first()
            .ok_or("owner")?
            .name,
        1
    );
    for bytes in [vec![0, 8, 1, 0, 0, 0x0a], vec![0, 8, 0, 1, 0]] {
        let source = programs::source(7, &[bytes], &[], 1)?;
        let (report, language) = run(&[source], None, ControlOptions::default())?;
        assert_eq!(
            report.files.first().ok_or("file")?.status,
            LoadStatus::Activated
        );
        assert!(language.currency.is_none());
    }
    let malformed = programs::source(7, &[vec![0, 8, 1, 1, 46, 0x0a, 0]], &[], 1)?;
    let (report, language) = run(&[malformed], None, ControlOptions::default())?;
    let file = report.files.first().ok_or("file")?;
    assert_eq!(file.status, LoadStatus::Disabled);
    assert!(matches!(
        file.errors.first().ok_or("error")?.failure,
        super::LoadFailure::ReadBounds
    ));
    assert!(language.currency.is_none());
    Ok(())
}
