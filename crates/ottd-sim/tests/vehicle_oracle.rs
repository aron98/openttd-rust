//! Exact replay of state captured around original native vehicle callbacks.
use ottd_sim::{VehicleCallbacks, VehicleOperation, run_vehicle_callback};
use serde::Deserialize;

#[derive(Deserialize)]
struct Oracle {
    schema_version: u32,
    vehicle_cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    before: VehicleCallbacks,
    operation: VehicleOperation,
    after: VehicleCallbacks,
}

#[test]
fn original_vehicle_callbacks_match_every_captured_field() -> Result<(), Box<dyn std::error::Error>>
{
    // Given fresh native output when supplied, otherwise the committed native corpus.
    let path = std::env::var("OTTD_CALLBACK_JSON").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference/callbacks.json"
        )
        .to_owned()
    });
    let oracle: Oracle = serde_json::from_reader(std::fs::File::open(path)?)?;
    assert_eq!(oracle.schema_version, 1);
    assert!(oracle.vehicle_cases.len() >= 10);
    for case in oracle.vehicle_cases {
        // When Rust runs the same original callback boundary.
        let actual = run_vehicle_callback(case.before, case.operation)?;
        // Then every vehicle, cache, context, and RNG field matches native output.
        assert_eq!(actual, case.after, "{}", case.name);
    }
    Ok(())
}
