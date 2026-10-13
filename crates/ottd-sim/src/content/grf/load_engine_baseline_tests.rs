use super::{ControlOptions, LoadStatus, Result, programs, run};

fn property(kind: u8, raw: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, 1, 1, 1, 255, 0, 0, kind];
    bytes.extend_from_slice(raw);
    bytes
}

fn accepted(kind: u8, value: u8) -> Result {
    let source = programs::source(0x454e_4701, &[property(kind, &[value])], &[], 1)?;
    let (report, _) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        report.files.first().ok_or("file")?.status,
        LoadStatus::Activated
    );
    Ok(())
}

#[test]
fn engine_baseline_speed_is_accepted() -> Result {
    accepted(0x08, 80)
}

#[test]
fn engine_baseline_running_cost_is_accepted() -> Result {
    accepted(0x09, 7)
}

#[test]
fn engine_baseline_capacity_is_accepted() -> Result {
    accepted(0x0f, 42)
}

#[test]
fn engine_baseline_cost_factor_is_accepted() -> Result {
    accepted(0x11, 11)
}

#[test]
fn engine_baseline_truncated_property_disables_file() -> Result {
    let source = programs::source(0x454e_4701, &[property(0x08, &[])], &[], 1)?;
    let (report, _) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        report.files.first().ok_or("file")?.status,
        LoadStatus::Disabled
    );
    Ok(())
}

#[test]
fn engine_baseline_unknown_property_disables_file() -> Result {
    let source = programs::source(0x454e_4701, &[property(0x01, &[99])], &[], 1)?;
    let (report, _) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        report.files.first().ok_or("file")?.status,
        LoadStatus::Disabled
    );
    Ok(())
}
