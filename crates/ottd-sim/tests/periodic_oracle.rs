//! Complete original callback fixtures replayed through the typed operation envelope.
use ottd_sim::{CallbackFixture, simulate_callback};
use serde::Deserialize;

#[derive(Deserialize)]
struct Oracle {
    schema_version: u32,
    periodic_cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    before: serde_json::Value,
    operation: serde_json::Value,
    after: serde_json::Value,
}
#[test]
fn original_periodic_callbacks_match_all_fields() -> Result<(), Box<dyn std::error::Error>> {
    // Given the fresh original C++ callback corpus (or its committed copy).
    let path = std::env::var("OTTD_CALLBACK_JSON").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference/callbacks.json"
        )
        .to_owned()
    });
    let oracle: Oracle = serde_json::from_reader(std::fs::File::open(path)?)?;
    assert_eq!(oracle.schema_version, 1);
    assert_eq!(oracle.periodic_cases.len(), 8);
    for case in oracle.periodic_cases {
        let kind = case.operation.get("kind").ok_or("missing kind")?;
        let before: CallbackFixture = serde_json::from_value(
            serde_json::json!({"schema_version":1,"callback":{"kind":kind,"state":case.before}}),
        )?;
        let after: CallbackFixture = serde_json::from_value(
            serde_json::json!({"schema_version":1,"callback":{"kind":kind,"state":case.after}}),
        )?;
        // When the typed original callback boundary executes.
        let actual = simulate_callback(before)?;
        // Then every raw map, table, status, preserved rating, and RNG field agrees.
        assert_eq!(actual, after, "{}", case.name);
    }
    Ok(())
}
