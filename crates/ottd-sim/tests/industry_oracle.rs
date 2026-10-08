//! Original registered industry callback parity, including pre-reset timer phase.
use ottd_sim::{IndustryCallbacks, IndustryMonth, run_industry_month};
use serde::Deserialize;

#[derive(Deserialize)]
struct Oracle {
    schema_version: u32,
    industry_cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    before: IndustryCallbacks,
    operation: Operation,
    after: IndustryCallbacks,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    IndustryMonth { phase: IndustryMonth },
}
#[test]
fn original_industry_callback_matches_all_records() -> Result<(), Box<dyn std::error::Error>> {
    // Given native captures made around the original registered callback.
    let path = std::env::var("OTTD_CALLBACK_JSON").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference/callbacks.json"
        )
        .to_owned()
    });
    let oracle: Oracle = serde_json::from_reader(std::fs::File::open(path)?)?;
    assert_eq!(oracle.schema_version, 1);
    assert_eq!(oracle.industry_cases.len(), 14);
    for case in oracle.industry_cases {
        let Operation::IndustryMonth { phase } = case.operation;
        // When Rust receives the actual pre-reset callback phase.
        let actual = run_industry_month(case.before, phase)?;
        // Then every record, mask bit, accumulator, builder value and RNG agrees.
        assert_eq!(actual, case.after, "{}", case.name);
    }
    Ok(())
}
