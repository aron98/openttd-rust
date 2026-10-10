use super::{ControlOptions, LoadStatus, Result, engine_units, programs, run};
use serde_json::Value;

const GRFID: u32 = 0x4341_5201;

fn property(feature: u8, first: u16, kind: u8, count: u8, raw: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, feature, 1, count, 255];
    bytes.extend(first.to_le_bytes());
    bytes.push(kind);
    bytes.extend(raw);
    bytes
}

fn accepted(records: &[Vec<u8>]) -> Result {
    let source = programs::source(GRFID, records, &[], 1)?;
    let (control, _) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        control.files.first().ok_or("file")?.status,
        LoadStatus::Activated
    );
    Ok(())
}

#[test]
fn cargo_red_existing_road08_control() -> Result {
    accepted(&[property(1, 88, 8, 1, &[80])])
}

#[test]
fn cargo_red_reserve_bitnum_accepts_native_slot12() -> Result {
    accepted(&[property(11, 12, 8, 1, &[12])])
}

#[test]
fn cargo_red_reserve_label_accepts_native_qaaa() -> Result {
    accepted(&[property(11, 12, 23, 1, b"QAAA")])
}

#[test]
fn cargo_red_global_translation_accepts_native_explicit_table() -> Result {
    accepted(&[property(8, 0, 9, 2, b"PASSQAAA")])
}

#[test]
fn cargo_red_road_default_accepts_native_invalid_cargo() -> Result {
    accepted(&[property(1, 88, 16, 1, &[255])])
}

#[test]
fn cargo_red_identity_translation_road_chain_retains_native_owner() -> Result {
    let records = [
        property(11, 12, 8, 1, &[12]),
        property(11, 12, 23, 1, b"QAAA"),
        property(8, 0, 9, 2, b"PASSQAAA"),
        property(1, 88, 16, 1, &[1]),
    ];
    let source = programs::source(GRFID, &records, &[], 1)?;
    let (control, language) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        control.files.first().ok_or("file")?.status,
        LoadStatus::Activated
    );
    let actual = engine_units::projection(&language.specs.ok_or("engine state")?)?;
    let expected: Value = serde_json::from_str(include_str!("load_cargo_native_owner.json"))?;
    let owner = actual
        .get("owners")
        .and_then(Value::as_array)
        .ok_or("owners")?
        .iter()
        .find(|value| value.get("id").and_then(Value::as_u64) == Some(256))
        .ok_or("owner256")?;
    assert_eq!(owner, expected.get("owner").ok_or("native owner")?);
    let temporary = actual
        .get("temporary")
        .and_then(Value::as_array)
        .ok_or("temporary")?
        .get(256)
        .ok_or("temporary256")?;
    assert_eq!(
        temporary,
        expected.get("temporary").ok_or("native temporary")?
    );
    Ok(())
}
