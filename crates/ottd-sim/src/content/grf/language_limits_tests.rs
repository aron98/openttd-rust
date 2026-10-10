use super::{Pack, Result, header};
use crate::content::grf;

#[test]
fn language_input_limits_refuse_before_parsing_and_admit_exact_source_size() -> Result {
    use grf::{
        load::{RuntimeInputs, run_with_context},
        load_language_state::{LanguageInput, LanguageLimits},
    };
    let pack = header();
    let packs = [pack.as_slice()];
    for (count, bytes, total, expected) in [
        (0, 572, 572, Some("language packs")),
        (1, 571, 572, Some("language source bytes")),
        (1, 572, 571, Some("source/config bytes")),
        (1, 572, 572, None),
    ] {
        let result = run_with_context(
            &[],
            &[],
            grf::ControlOptions {
                max_source_bytes: total,
                ..grf::ControlOptions::default()
            },
            RuntimeInputs {
                environment: None,
                language: Some(LanguageInput {
                    packs: &packs,
                    selected: 1,
                    limits: LanguageLimits {
                        packs: count,
                        source_bytes: bytes,
                        ..LanguageLimits::default()
                    },
                }),
            },
        );
        match expected {
            Some(expected) => assert!(
                matches!(result,Err(grf::ControlLoadError::ResourceLimit {resource,..}) if resource==expected)
            ),
            None => {
                result?;
            }
        }
    }
    Ok(())
}

#[test]
fn catalog_payload_is_charged_even_without_configured_files() -> Result {
    use grf::{
        load::{RuntimeInputs, run_with_context},
        load_language_state::{LanguageInput, LanguageLimits, LanguageReport},
    };
    let pack = header();
    let packs = [pack.as_slice()];
    let baseline_event_bytes = 4_usize
        .saturating_mul(std::mem::size_of::<
            grf::load_language_state::LanguageSnapshot,
        >())
        .saturating_add(12_usize.saturating_mul(std::mem::size_of::<grf::LoadEvent>()));
    let options = grf::ControlOptions {
        max_trace_bytes: baseline_event_bytes
            .saturating_add(std::mem::size_of::<Pack>())
            .saturating_add(std::mem::size_of::<LanguageReport>())
            .saturating_add(std::mem::size_of::<&str>())
            .saturating_sub(1),
        ..grf::ControlOptions::default()
    };
    let result = run_with_context(
        &[],
        &[],
        options,
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected: 1,
                limits: LanguageLimits::default(),
            }),
        },
    );
    assert!(matches!(
        result,
        Err(grf::ControlLoadError::ResourceLimit {
            resource: "trace payload bytes",
            ..
        })
    ));
    run_with_context(
        &[],
        &[],
        grf::ControlOptions {
            max_trace_bytes: options.max_trace_bytes.saturating_add(1),
            ..options
        },
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected: 1,
                limits: LanguageLimits::default(),
            }),
        },
    )?;
    Ok(())
}

#[test]
fn a_new_loader_session_does_not_retain_prior_language_maps() -> Result {
    use grf::{
        load::{RuntimeInputs, run_with_context},
        load_language_state::{LanguageInput, LanguageLimits},
    };
    let pack = header();
    let packs = [pack.as_slice()];
    let context = RuntimeInputs {
        environment: None,
        language: Some(LanguageInput {
            packs: &packs,
            selected: 1,
            limits: LanguageLimits::default(),
        }),
    };
    let first = grf::load_context_tests::grf_control_cases::source(
        0x4141_4141,
        &[vec![0, 8, 1, 1, 1, 0x15, 3]],
        &[],
        1,
    )?;
    let second = grf::load_context_tests::grf_control_cases::source(0x4141_4141, &[], &[], 1)?;
    let (_, _, first_report) = run_with_context(
        &[first.input()],
        &[],
        grf::ControlOptions::default(),
        context,
    )?;
    let (_, _, second_report) = run_with_context(
        &[second.input()],
        &[],
        grf::ControlOptions::default(),
        context,
    )?;
    let first_report = first_report.ok_or("first report")?;
    let second_report = second_report.ok_or("second report")?;
    assert!(
        first_report
            .events
            .last()
            .and_then(|event| event.files.first())
            .is_some_and(|file| file.maps.contains_key(&1))
    );
    assert!(
        second_report
            .events
            .iter()
            .all(|event| event.files.iter().all(|file| file.maps.is_empty()))
    );
    Ok(())
}

#[test]
fn mapping_pair_budget_counts_repeated_entries_not_unique_keys() -> Result {
    use grf::{
        load::{RuntimeInputs, run_with_context},
        load_language_state::{LanguageInput, LanguageLimits},
    };
    let pack = header();
    let packs = [pack.as_slice()];
    let program = grf::load_context_tests::grf_control_cases::source(
        0x4141_4141,
        &[vec![0, 8, 1, 1, 1, 0x13, 1, 0, 1, 0, 0]],
        &[],
        1,
    )?;
    for limit in [1, 2] {
        let result = run_with_context(
            &[program.input()],
            &[],
            grf::ControlOptions::default(),
            RuntimeInputs {
                environment: None,
                language: Some(LanguageInput {
                    packs: &packs,
                    selected: 1,
                    limits: LanguageLimits {
                        map_pairs: limit,
                        ..LanguageLimits::default()
                    },
                }),
            },
        );
        if limit == 1 {
            assert!(matches!(
                result,
                Err(grf::ControlLoadError::ResourceLimit {
                    resource: "language mapping pairs",
                    ..
                })
            ));
        } else {
            result?;
        }
    }
    Ok(())
}

#[test]
fn distinct_active_files_with_one_grfid_refuse_native_activation_invariant() -> Result {
    use grf::{
        load::{RuntimeInputs, run_with_context},
        load_language_state::{LanguageInput, LanguageLimits},
    };
    let pack = header();
    let packs = [pack.as_slice()];
    let first = grf::load_context_tests::grf_control_cases::source(0x4141_4141, &[], &[], 1)?;
    let mut second = grf::load_context_tests::grf_control_cases::source(0x4141_4141, &[], &[], 1)?;
    second.name = "second.grf".into();
    let error = run_with_context(
        &[first.input(), second.input()],
        &[],
        grf::ControlOptions::default(),
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected: 1,
                limits: LanguageLimits::default(),
            }),
        },
    )
    .err()
    .ok_or("invalid active alias admitted")?;
    assert!(matches!(
        error,
        grf::ControlLoadError::InvalidNativeDomain {
            detail: "activation first-GRFID file invariant",
            ..
        }
    ));
    Ok(())
}
