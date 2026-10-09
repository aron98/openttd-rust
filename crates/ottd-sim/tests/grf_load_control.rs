//! Original loading phases and parameter control, without catalog activation.
use ottd_sim::content::grf::{
    ControlLoadError, ControlOptions, GrfIdentity, LoadEvent, LoadFlags, LoadInput, LoadStage,
    LoadStatus, Palette, run_control_load, run_control_load_with_prefix,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn file(id: [u8; 4], records: &[Vec<u8>]) -> Result<Vec<u8>> {
    let mut name = vec![8, 8];
    name.extend(id);
    name.extend(b"control\0");
    let mut bytes = vec![4, 0, 255, 0, 0, 0, 0];
    for record in std::iter::once(&name).chain(records) {
        bytes.extend(u16::try_from(record.len())?.to_le_bytes());
        bytes.push(255);
        bytes.extend(record);
    }
    bytes.extend([0, 0]);
    Ok(bytes)
}
fn input<'a>(
    name: &'a str,
    bytes: Option<&'a [u8]>,
    id: [u8; 4],
    parameters: &'a [u32],
) -> LoadInput<'a> {
    LoadInput {
        name,
        bytes,
        identity: GrfIdentity {
            grfid: u32::from_le_bytes(id),
            md5: [0; 16],
        },
        metadata_version: 0,
        palette: Palette::Windows,
        parameters,
        flags: LoadFlags::default(),
    }
}

#[test]
fn all_files_advance_one_phase_before_any_enters_the_next() -> Result {
    let a = file(*b"AAAA", &[vec![0x0d, 0, 1, 0, 255, 1, 0, 0, 0]])?;
    let b = file(*b"BBBB", &[vec![0x0d, 0, 1, 0, 255, 1, 0, 0, 0]])?;
    let report = run_control_load(
        &[
            input("a", Some(&a), *b"AAAA", &[0]),
            input("b", Some(&b), *b"BBBB", &[0]),
        ],
        ControlOptions::default(),
    )?;
    let order: Vec<_> = report
        .events
        .iter()
        .filter_map(|event| {
            if let LoadEvent::Parameters { location, .. } = event {
                Some((location.stage, location.file))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        order,
        vec![
            (LoadStage::Init, 0),
            (LoadStage::Init, 1),
            (LoadStage::Reserve, 0),
            (LoadStage::Reserve, 1),
            (LoadStage::Activation, 0),
            (LoadStage::Activation, 1)
        ]
    );
    for state in report.files {
        assert_eq!(state.parameters, Some(vec![3]));
        assert_eq!(state.status, LoadStatus::Activated);
        assert!(!state.reserved);
    }
    Ok(())
}

#[test]
fn setting_a_higher_parameter_defines_zero_filled_lower_entries() -> Result {
    let bytes = file(
        *b"AAAA",
        &[
            vec![0x0d, 3, 0, 255, 255, 7, 0, 0, 0],
            vec![0x0d, 0, 0x80, 255, 255, 9, 0, 0, 0],
        ],
    )?;
    let report = run_control_load(
        &[input("a", Some(&bytes), *b"AAAA", &[])],
        ControlOptions::default(),
    )?;
    assert_eq!(
        report.files.first().ok_or("file")?.parameters,
        Some(vec![0, 0, 0, 7])
    );
    Ok(())
}

#[test]
fn missing_source_retains_config_identity_without_fabricating_a_dynamic_file() -> Result {
    let report = run_control_load(
        &[input("missing", None, *b"MISS", &[5])],
        ControlOptions::default(),
    )?;
    let state = report.files.first().ok_or("missing config")?;
    assert_eq!(state.config_grfid, u32::from_le_bytes(*b"MISS"));
    assert_eq!(state.status, LoadStatus::NotFound);
    assert!(state.file_grfid.is_none());
    assert!(state.parameters.is_none());
    Ok(())
}

#[test]
fn labels_are_collected_before_execution_and_cleared_after_activation() -> Result {
    let bytes = file(*b"AAAA", &[vec![0x10, 42]])?;
    let report = run_control_load(
        &[input("a", Some(&bytes), *b"AAAA", &[])],
        ControlOptions::default(),
    )?;
    let collected = report
        .events
        .iter()
        .find_map(|event| match event {
            LoadEvent::StageEnd {
                stage: LoadStage::LabelScan,
                files,
                ..
            } => files.first(),
            _ => None,
        })
        .ok_or("label snapshot")?;
    let labels = collected.labels.as_ref().ok_or("dynamic labels")?;
    assert_eq!(labels.len(), 1);
    assert_eq!(labels.first().ok_or("label")?.id, 42);
    assert_eq!(labels.first().ok_or("label")?.line, 2);
    assert_eq!(report.files.first().ok_or("file")?.labels, Some(Vec::new()));
    Ok(())
}

#[test]
fn action6_mutations_accumulate_across_all_three_executable_phases() -> Result {
    let bytes = file(
        *b"AAAA",
        &[
            vec![6, 0, 0x84, 5, 255],
            vec![0x0d, 1, 0, 255, 255, 10, 0, 0, 0],
        ],
    )?;
    let report = run_control_load(
        &[input("a", Some(&bytes), *b"AAAA", &[1])],
        ControlOptions::default(),
    )?;
    assert_eq!(
        report.files.first().ok_or("file")?.parameters,
        Some(vec![1, 13])
    );
    let values: Vec<_> = report
        .events
        .iter()
        .filter_map(|event| match event {
            LoadEvent::Override { bytes, .. } => bytes.get(5).copied(),
            _ => None,
        })
        .collect();
    assert_eq!(values, vec![11, 12, 13]);
    Ok(())
}

#[test]
fn forward_label_jump_skips_executed_unsupported_action() -> Result {
    let bytes = file(
        *b"AAAA",
        &[
            vec![9, 0, 1, 2, 1, 42],
            vec![0],
            vec![0x10, 42],
            vec![0x0d, 1, 0, 255, 255, 7, 0, 0, 0],
        ],
    )?;
    let report = run_control_load(
        &[input("a", Some(&bytes), *b"AAAA", &[1])],
        ControlOptions::default(),
    )?;
    assert_eq!(
        report.files.first().ok_or("file")?.parameters,
        Some(vec![1, 7])
    );
    let destinations: Vec<_> = report
        .events
        .iter()
        .filter_map(|event| match event {
            LoadEvent::Jump { target_line, .. } => Some(*target_line),
            _ => None,
        })
        .collect();
    assert_eq!(destinations, vec![4, 4, 4]);
    Ok(())
}

#[test]
fn identity_prefix_refuses_masked_config_and_exact_dynamic_reads() -> Result {
    for record in [
        vec![9, 0x88, 8, 9, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        vec![0x0d, 0, 0, 0, 254, 0x42, 0x42, 0x42, 0x42],
    ] {
        let bytes = file(*b"AAAA", &[record])?;
        let inputs = [input("a", Some(&bytes), *b"AAAA", &[])];
        assert!(run_control_load(&inputs, ControlOptions::default()).is_ok());
        assert!(matches!(
            run_control_load_with_prefix(&inputs, &[0x4242_4242], ControlOptions::default()),
            Err(ControlLoadError::Unsupported {
                detail: "lookup selects unexecuted preceding config or dynamic file",
                ..
            })
        ));
    }
    Ok(())
}

#[test]
fn native_arithmetic_and_identity_assertion_domains_are_refused() -> Result {
    for operation in [5, 6, 10, 12] {
        let bytes = file(*b"AAAA", &[vec![0x0d, 2, operation, 0, 1]])?;
        let parameters = if operation < 7 {
            [0x8000_0000, 0xffff_ffe0]
        } else {
            [0x8000_0000, u32::MAX]
        };
        assert!(matches!(
            run_control_load(
                &[input("a", Some(&bytes), *b"AAAA", &parameters)],
                ControlOptions::default()
            ),
            Err(ControlLoadError::InvalidNativeDomain { .. })
        ));
    }
    let bytes = file(*b"BBBB", &[])?;
    assert!(matches!(
        run_control_load(
            &[input("a", Some(&bytes), *b"AAAA", &[])],
            ControlOptions::default()
        ),
        Err(ControlLoadError::InvalidNativeDomain {
            detail: "activation first-GRFID file invariant",
            ..
        })
    ));
    Ok(())
}

#[test]
fn cumulative_limits_refuse_work_without_fabricating_disabled_reports() -> Result {
    let bytes = file(
        *b"AAAA",
        &[
            vec![0x10, 42],
            vec![6, 0, 4, 5, 255],
            vec![0x0d, 1, 0, 255, 255, 0, 0, 0, 0],
        ],
    )?;
    let inputs = [input("a", Some(&bytes), *b"AAAA", &[1])];
    let source_before = bytes.clone();
    for options in [
        ControlOptions {
            max_steps: 2,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_labels: 0,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_override_bytes: 1,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_trace_events: 2,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_trace_bytes: 64,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_files: 0,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_source_bytes: 1,
            ..ControlOptions::default()
        },
    ] {
        assert!(matches!(
            run_control_load(&inputs, options),
            Err(ControlLoadError::ResourceLimit { .. })
        ));
        assert_eq!(bytes, source_before);
    }
    let loop_bytes = file(*b"AAAA", &[vec![0x10, 42], vec![9, 0, 1, 2, 1, 42]])?;
    assert!(matches!(
        run_control_load(
            &[input("loop", Some(&loop_bytes), *b"AAAA", &[1])],
            ControlOptions {
                max_steps: 20,
                ..ControlOptions::default()
            }
        ),
        Err(ControlLoadError::ResourceLimit {
            resource: "record visits",
            ..
        })
    ));
    Ok(())
}

#[test]
fn zero_length_action6_lookahead_retains_the_original_empty_override() -> Result {
    let mut bytes = file(*b"AAAA", &[vec![6, 255]])?;
    bytes.push(255);
    let report = run_control_load(
        &[input("a", Some(&bytes), *b"AAAA", &[])],
        ControlOptions::default(),
    )?;
    let states = report
        .events
        .iter()
        .filter_map(|event| match event {
            LoadEvent::StageEnd {
                stage: LoadStage::Init | LoadStage::Reserve | LoadStage::Activation,
                overrides,
                ..
            } => Some(overrides),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(states.len(), 3);
    for overrides in states {
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides.first().ok_or("empty override")?.line, 3);
        assert!(overrides.first().ok_or("empty override")?.bytes.is_empty());
    }
    Ok(())
}

#[test]
fn unsupported_executed_families_and_structural_errors_are_distinct_from_native_disable() -> Result
{
    for record in [
        vec![0],
        vec![1, 0, 1, 0],
        vec![2],
        vec![3],
        vec![4],
        vec![5, 0, 0],
        vec![0x0a, 0],
        vec![0x0b],
        vec![0x0e],
        vec![0x0f],
        vec![0x11, 0, 0],
        vec![0x12, 0],
        vec![0x13],
        vec![0x0d, 0, 0, 0x85, 255],
        vec![0x0d, 0, 0, 0, 254, 255, 255, 0, 0],
    ] {
        let bytes = file(*b"AAAA", &[record])?;
        assert!(matches!(
            run_control_load(
                &[input("a", Some(&bytes), *b"AAAA", &[])],
                ControlOptions::default()
            ),
            Err(ControlLoadError::Unsupported { .. })
        ));
    }
    let mut bytes = file(*b"AAAA", &[vec![0x0d]])?;
    let report = run_control_load(
        &[input("a", Some(&bytes), *b"AAAA", &[])],
        ControlOptions::default(),
    )?;
    assert_eq!(
        report.files.first().ok_or("disabled file")?.status,
        LoadStatus::Disabled
    );
    bytes.pop();
    assert!(matches!(
        run_control_load(
            &[input("a", Some(&bytes), *b"AAAA", &[])],
            ControlOptions::default()
        ),
        Err(ControlLoadError::Structural { .. })
    ));
    Ok(())
}

#[test]
fn exact_work_limits_admit_the_last_unit_and_reject_the_next() -> Result {
    let bytes = file(
        *b"AAAA",
        &[
            vec![0x10, 42],
            vec![6, 0, 4, 5, 255],
            vec![0x0d, 1, 0, 255, 255, 0, 0, 0, 0],
        ],
    )?;
    let inputs = [input("a", Some(&bytes), *b"AAAA", &[1])];
    let report = run_control_load(&inputs, ControlOptions::default())?;
    let steps = report
        .events
        .iter()
        .filter(|event| matches!(event, LoadEvent::Record { .. }))
        .count();
    let source_bytes = bytes.len().saturating_add(5);
    let bounds = [
        ControlOptions {
            max_steps: steps,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_trace_events: report.events.len(),
            ..ControlOptions::default()
        },
        ControlOptions {
            max_source_bytes: source_bytes,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_override_bytes: 21,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_labels: 1,
            ..ControlOptions::default()
        },
        ControlOptions {
            max_files: 1,
            ..ControlOptions::default()
        },
    ];
    for exact in bounds {
        assert!(run_control_load(&inputs, exact).is_ok());
    }
    for short in [
        ControlOptions {
            max_steps: steps.saturating_sub(1),
            ..ControlOptions::default()
        },
        ControlOptions {
            max_trace_events: report.events.len().saturating_sub(1),
            ..ControlOptions::default()
        },
        ControlOptions {
            max_source_bytes: source_bytes.saturating_sub(1),
            ..ControlOptions::default()
        },
        ControlOptions {
            max_override_bytes: 20,
            ..ControlOptions::default()
        },
    ] {
        assert!(matches!(
            run_control_load(&inputs, short),
            Err(ControlLoadError::ResourceLimit { .. })
        ));
    }
    Ok(())
}

#[test]
fn duplicate_filename_exposes_first_dynamic_file_before_second_config_visit() -> Result {
    let bytes = file(*b"AAAA", &[])?;
    let report = run_control_load(
        &[
            input("same", Some(&bytes), *b"AAAA", &[11]),
            input("same", Some(&bytes), *b"AAAA", &[99]),
        ],
        ControlOptions::default(),
    )?;
    let states = report
        .events
        .iter()
        .find_map(|event| match event {
            LoadEvent::Decision { files, .. } => Some(files),
            _ => None,
        })
        .ok_or("first decision")?;
    assert_eq!(
        states.get(1).ok_or("second config")?.parameters,
        Some(vec![11])
    );
    assert_eq!(
        states.get(1).ok_or("second config")?.file_grfid,
        Some(u32::from_le_bytes(*b"AAAA"))
    );
    let different = file(*b"BBBB", &[])?;
    assert!(matches!(
        run_control_load(
            &[
                input("same", Some(&bytes), *b"AAAA", &[]),
                input("same", Some(&different), *b"BBBB", &[])
            ],
            ControlOptions::default()
        ),
        Err(ControlLoadError::InvalidNativeDomain {
            detail: "one filename supplies inconsistent immutable source bytes",
            ..
        })
    ));
    Ok(())
}

#[test]
fn trace_limit_charges_the_emitted_event_storage() {
    let bytes = 8_usize.saturating_mul(std::mem::size_of::<LoadEvent>());
    assert!(
        run_control_load(
            &[],
            ControlOptions {
                max_trace_bytes: bytes,
                ..ControlOptions::default()
            }
        )
        .is_ok()
    );
    assert!(matches!(
        run_control_load(
            &[],
            ControlOptions {
                max_trace_bytes: bytes.saturating_sub(1),
                ..ControlOptions::default()
            }
        ),
        Err(ControlLoadError::ResourceLimit {
            resource: "trace payload bytes",
            ..
        })
    ));
}

#[test]
fn standalone_mask_zero_and_forward_config_queries_obey_order() -> Result {
    let mut masked = vec![9, 0x88, 8, 8];
    masked.extend([0; 8]);
    masked.push(1);
    let a = file(*b"AAAA", &[masked, vec![0x0d, 0, 1, 0, 255, 1, 0, 0, 0]])?;
    let report = run_control_load(
        &[input("a", Some(&a), *b"AAAA", &[0])],
        ControlOptions::default(),
    )?;
    assert_eq!(
        report
            .files
            .first()
            .ok_or("first masked config")?
            .parameters,
        Some(vec![2])
    );
    let mut forward = vec![9, 0x88, 4, 8];
    forward.extend(*b"BBBB");
    forward.push(1);
    let a = file(*b"AAAA", &[forward, vec![0x0d, 0, 1, 0, 255, 1, 0, 0, 0]])?;
    let b = file(*b"BBBB", &[])?;
    for (reverse, expected) in [(false, 1), (true, 2)] {
        let mut inputs = vec![
            input("a", Some(&a), *b"AAAA", &[0]),
            input("b", Some(&b), *b"BBBB", &[]),
        ];
        if reverse {
            inputs.reverse();
        }
        let report = run_control_load(&inputs, ControlOptions::default())?;
        assert_eq!(
            report
                .files
                .iter()
                .find(|state| state.config_grfid == u32::from_le_bytes(*b"AAAA"))
                .ok_or("querier")?
                .parameters,
            Some(vec![expected])
        );
    }
    Ok(())
}
