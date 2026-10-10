use super::{Input, check};
use ottd_save::{Savegame, world::World};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
struct Probe {
    schema_version: u8,
    foundation_price: i64,
    rows: Vec<Row>,
    price_probes: Vec<PriceProbe>,
    generic_error_id: u16,
    invalid_error_id: u16,
    before: Value,
    after: Value,
    live_before: Value,
    live_after: Value,
}
#[derive(Debug, Deserialize)]
struct Row {
    slope: u8,
    requested: u8,
    existing: u8,
    other: u8,
    enabled: bool,
    #[serde(flatten)]
    outcome: Outcome,
}
#[derive(Debug, Deserialize)]
struct PriceProbe {
    price: i64,
    result: Row,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Outcome {
    pieces: u8,
    success: bool,
    cost: i64,
    expenses: u8,
    error_id: u16,
}
fn compare(row: &Row, price: i64, probe: &Probe) -> Result {
    let actual = check(Input::new(
        row.slope,
        row.requested,
        row.existing,
        row.other,
        row.enabled,
        price,
    )?);
    let error_id = match actual.cost.error.as_deref() {
        None => probe.invalid_error_id,
        Some("CMD_ERROR") => probe.generic_error_id,
        Some(_) => return Err("unexpected road slope error symbol".into()),
    };
    let actual = Outcome {
        pieces: actual.pieces,
        success: actual.cost.success,
        cost: actual.cost.cost,
        expenses: actual.cost.expenses,
        error_id,
    };
    assert_eq!(actual, row.outcome, "native road slope output differs");
    Ok(())
}
fn saved(path: &Path) -> Result<Value> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?
    .saved_json()?)
}
fn live(path: &Path) -> Result<Value> {
    let value: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    Ok(
        json!({"depot": value.get("runtime").ok_or("depot runtime")?, "vehicles": value.get("vehicles").ok_or("road runtime")?}),
    )
}

#[test]
#[ignore = "requires original CheckRoadSlope vectors and canonical save witnesses"]
fn original_road_slope_matrix() -> Result {
    let root = PathBuf::from(std::env::var("ROAD_SLOPE_CASE")?);
    let probe: Probe =
        serde_json::from_slice(&std::fs::read(root.join("vectors/road-slope.json"))?)?;
    assert_eq!(probe.schema_version, 1);
    assert_eq!(probe.rows.len(), 155_648);
    assert_eq!(probe.before, probe.after);
    assert_eq!(probe.live_before, probe.live_after);
    let canonical = saved(&root.join("canonical/save/autosave/exit.sav"))?;
    for folder in ["vectors", "reload"] {
        assert_eq!(
            saved(&root.join(folder).join("save/autosave/exit.sav"))?,
            canonical,
            "road slope probes changed full saved state"
        );
        assert_eq!(
            live(&root.join(folder).join("depot-runtime.json"))?,
            probe.live_before
        );
    }
    assert_eq!(
        live(&root.join("canonical/depot-runtime.json"))?,
        probe.live_before
    );
    let mut rows = probe.rows.iter();
    for slope in [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 23, 27, 29, 30,
    ] {
        for requested in 0..16 {
            for existing in 0..16 {
                for other in 0..16 {
                    for enabled in [false, true] {
                        let row = rows.next().ok_or("missing original row")?;
                        assert_eq!(
                            (
                                row.slope,
                                row.requested,
                                row.existing,
                                row.other,
                                row.enabled
                            ),
                            (slope, requested, existing, other, enabled)
                        );
                        compare(row, probe.foundation_price, &probe)?;
                    }
                }
            }
        }
    }
    assert!(rows.next().is_none());
    assert_eq!(probe.price_probes.len(), 5);
    for (price, observed) in [0, 1, -1, i64::MIN, i64::MAX]
        .into_iter()
        .zip(&probe.price_probes)
    {
        assert_eq!(price, observed.price);
        let row = &observed.result;
        assert_eq!(
            (
                row.slope,
                row.requested,
                row.existing,
                row.other,
                row.enabled
            ),
            (1, 1, 0, 0, true)
        );
        compare(row, price, &probe)?;
    }
    std::fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(
            &json!({"rows": probe.rows.len(), "price_probes": probe.price_probes.len(), "full_saved_live_equal": true, "settings_restored": true}),
        )?,
    )?;
    Ok(())
}
