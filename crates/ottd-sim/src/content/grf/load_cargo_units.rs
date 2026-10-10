use super::super::{
    ControlLoadError, LoadFailure, LoadLocation, LoadStage, load_budget::Budget,
    load_cargo::CargoState,
};
use super::{ControlOptions, LoadStatus, Result, programs, run};

#[test]
fn cargo_constructor_checks_budget_before_owner_allocation() {
    let mut budget = Budget::new(ControlOptions {
        max_trace_bytes: 1,
        ..ControlOptions::default()
    });
    let location = LoadLocation {
        stage: LoadStage::Reserve,
        file: 0,
        line: 1,
        offset: 0,
    };
    assert!(matches!(
        CargoState::new(crate::content::Climate::Temperate, 0, &mut budget, location),
        Err(ControlLoadError::ResourceLimit { .. })
    ));
}

#[test]
fn cargo_reset_preserves_two_distinct_inherited_standard_masks() -> Result {
    let mut budget = Budget::new(ControlOptions::default());
    let location = LoadLocation {
        stage: LoadStage::Reserve,
        file: 0,
        line: 1,
        offset: 0,
    };
    for inherited in [0_u64, u64::MAX] {
        let state = CargoState::new(
            crate::content::Climate::Temperate,
            inherited,
            &mut budget,
            location,
        )?;
        assert_eq!(state.standard_cargo_mask, inherited);
        assert_eq!(state.cargo_mask, 2047);
        assert_ne!(state.cargo_mask, inherited);
    }
    Ok(())
}

#[test]
fn cargo_unknown_stops_same_action_and_retains_prior_slot_write() -> Result {
    let action = vec![0, 11, 3, 1, 12, 8, 12, 1, 8, 99];
    let source = programs::source(0x4341_5201, &[action], &[], 1)?;
    let (control, report) = run(&[source], None, ControlOptions::default())?;
    let file = control.files.first().ok_or("file")?;
    assert_eq!(file.status, LoadStatus::Disabled);
    assert_eq!(
        file.errors.first().ok_or("diagnostic")?.failure,
        LoadFailure::UnknownProperty
    );
    let cargo = report.cargo.ok_or("cargo")?;
    assert_eq!(cargo.owners.get(12).ok_or("slot12")?.spec.bitnum, 12);
    Ok(())
}

#[test]
fn cargo_invalid_id_stops_same_action_without_consuming_later_table() -> Result {
    let action = vec![
        0, 8, 2, 1, 1, 9, b'P', b'A', b'S', b'S', 9, b'Q', b'A', b'A', b'A',
    ];
    let source = programs::source(0x4341_5201, &[action], &[], 1)?;
    let (control, _) = run(&[source], None, ControlOptions::default())?;
    let file = control.files.first().ok_or("file")?;
    assert_eq!(file.status, LoadStatus::Disabled);
    assert_eq!(
        file.errors.first().ok_or("diagnostic")?.failure,
        LoadFailure::InvalidId
    );
    Ok(())
}
