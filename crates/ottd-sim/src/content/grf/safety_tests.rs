use super::safety::{SafetyConfig, SafetyContext, SafetyLimits, SafetyOutcome, scan_safety};

fn file(actions: &[&[u8]]) -> Vec<u8> {
    let mut bytes = vec![4, 0, 255, 0, 0, 0, 0];
    for action in actions {
        bytes.extend(
            u16::try_from(action.len())
                .expect("small fixture")
                .to_le_bytes(),
        );
        bytes.push(255);
        bytes.extend_from_slice(action);
    }
    bytes.extend([0, 0]);
    bytes
}

#[test]
fn parameter_safety_reads_only_target() {
    let bytes = file(&[&[0x0d, 0x7f], &[0x0d, 0x9e]]);
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &[],
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(report.outcome, SafetyOutcome::Safe);
    assert_eq!(report.decisions.len(), 2);
}

#[test]
fn unsafe_action_stops_before_truncated_later_record() {
    let mut bytes = file(&[&[3]]);
    bytes.truncate(bytes.len() - 2);
    bytes.push(255);
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &[],
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(report.outcome, SafetyOutcome::Unsafe { line: 1, action: 3 });
    assert_eq!(report.decisions.len(), 1);
}

#[test]
fn ignored_action_does_not_parse_malformed_body() {
    let bytes = file(&[&[2, 255], &[0x0d, 0x80]]);
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &[],
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(
        report.outcome,
        SafetyOutcome::Unsafe {
            line: 2,
            action: 0x0d
        }
    );
}

#[test]
fn every_parameter_target_preserves_native_safety_classification() {
    for target in 0..=255 {
        let bytes = file(&[&[0x0d, target]]);
        let report = scan_safety(
            &bytes,
            SafetyContext {
                current_grfid: 1,
                configs: &[],
            },
            SafetyLimits::default(),
        )
        .expect("scan");
        let unsafe_target = target >= 0x80 && target != 0x9e;
        assert_eq!(
            matches!(report.outcome, SafetyOutcome::Unsafe { .. }),
            unsafe_target,
            "target {target}"
        );
    }
}

#[test]
fn mapping_uses_first_config_match() {
    let bytes = file(&[&[0, 8, 1, 1, 0, 0x11, 2, 0, 0, 0, 3, 0, 0, 0]]);
    let configs = [
        SafetyConfig {
            grfid: 2,
            is_static: true,
        },
        SafetyConfig {
            grfid: 2,
            is_static: false,
        },
    ];
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &configs,
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(report.outcome, SafetyOutcome::Safe);
}

#[test]
fn nonstatic_mapping_stops_before_missing_second_mapping() {
    let bytes = file(&[&[0, 8, 1, 2, 0, 0x11, 2, 0, 0, 0, 3, 0, 0, 0]]);
    let configs = [SafetyConfig {
        grfid: 2,
        is_static: false,
    }];
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &configs,
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(report.outcome, SafetyOutcome::Unsafe { line: 1, action: 0 });
    assert_eq!(report.failure, None);
}

#[test]
fn foreign_inhibit_stops_before_missing_second_id() {
    let bytes = file(&[&[0x0e, 2, 2, 0, 0, 0]]);
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &[],
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(
        report.outcome,
        SafetyOutcome::Unsafe {
            line: 1,
            action: 0x0e
        }
    );
    assert_eq!(report.failure, None);
}

#[test]
fn sprite_skip_hides_unsafe_action_until_count_expires() {
    let bytes = file(&[&[5, 0, 1], &[3], &[0x11]]);
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &[],
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(
        report.outcome,
        SafetyOutcome::Unsafe {
            line: 3,
            action: 0x11
        }
    );
    assert_eq!(
        report.decisions.get(1).expect("skipped record").action,
        None
    );
}

#[test]
fn read_bounds_disables_without_setting_unsafe() {
    let bytes = file(&[&[0x0d]]);
    let report = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &[],
        },
        SafetyLimits::default(),
    )
    .expect("scan");
    assert_eq!(report.outcome, SafetyOutcome::Safe);
    assert_eq!(report.failure, Some((super::ScanFailure::ReadBounds, 1)));
}

#[test]
fn static_wrapper_preserves_identity_when_disabled_but_not_unsafe() {
    let bytes = file(&[&[8, 8, 1, 0, 0, 0, b'n', 0, b'd', 0], &[0x0d]]);
    let original = super::scan_file(&bytes).expect("filescan");
    let result = super::scan_static_file(
        &bytes,
        super::ScanOptions::default(),
        &[],
        SafetyLimits::default(),
    )
    .expect("static scan");
    assert!(result.scan.accepted);
    assert_eq!(result.scan.identity, original.identity);
    assert_eq!(result.scan.metadata, original.metadata);
    assert_eq!(result.scan.status, super::ScanStatus::Disabled);
}

#[test]
fn static_wrapper_removes_checksum_identity_when_unsafe() {
    let bytes = file(&[&[8, 8, 1, 0, 0, 0, b'n', 0], &[3]]);
    let result = super::scan_static_file(
        &bytes,
        super::ScanOptions::default(),
        &[],
        SafetyLimits::default(),
    )
    .expect("static scan");
    assert!(!result.scan.accepted);
    assert_eq!(result.scan.identity, None);
    assert_eq!(result.scan.status, super::ScanStatus::Unknown);
}

#[test]
fn static_wrapper_does_not_enter_safety_for_system_identity() {
    let bytes = file(&[&[8, 8, 255, 0, 0, 0, b'n', 0], &[3]]);
    let result = super::scan_static_file(
        &bytes,
        super::ScanOptions::default(),
        &[],
        SafetyLimits {
            records: 0,
            ..SafetyLimits::default()
        },
    )
    .expect("static scan");
    assert!(!result.scan.accepted);
    assert_eq!(result.safety, None);
}

#[test]
fn decision_budget_rejects_before_allocating_beyond_bound() {
    let bytes = file(&[&[2], &[2]]);
    let result = scan_safety(
        &bytes,
        SafetyContext {
            current_grfid: 1,
            configs: &[],
        },
        SafetyLimits {
            trace_bytes: std::mem::size_of::<super::SafetyDecision>(),
            ..SafetyLimits::default()
        },
    );
    assert!(matches!(
        result,
        Err(super::SafetyError::Limit("trace bytes"))
    ));
}

#[test]
fn host_bounds_accept_exact_work_and_refuse_one_less() {
    let bytes = file(&[&[0, 8, 1, 1, 0, 0x11, 2, 0, 0, 0, 3, 0, 0, 0]]);
    let configs = [
        SafetyConfig {
            grfid: 7,
            is_static: true,
        },
        SafetyConfig {
            grfid: 2,
            is_static: true,
        },
    ];
    let context = SafetyContext {
        current_grfid: 1,
        configs: &configs,
    };
    let exact = SafetyLimits {
        bytes: bytes.len(),
        records: 1,
        registry_work: 2,
        trace_bytes: std::mem::size_of::<super::SafetyDecision>(),
    };
    assert!(scan_safety(&bytes, context, exact).is_ok());
    for (limits, resource) in [
        (
            SafetyLimits {
                bytes: bytes.len().saturating_sub(1),
                ..exact
            },
            "source bytes",
        ),
        (
            SafetyLimits {
                records: 0,
                ..exact
            },
            "records",
        ),
        (
            SafetyLimits {
                registry_work: 1,
                ..exact
            },
            "registry work",
        ),
        (
            SafetyLimits {
                trace_bytes: exact.trace_bytes.saturating_sub(1),
                ..exact
            },
            "trace bytes",
        ),
    ] {
        assert!(
            matches!(scan_safety(&bytes,context,limits),Err(super::SafetyError::Limit(found)) if found == resource)
        );
    }
}
