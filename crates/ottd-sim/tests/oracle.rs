//! Replay the committed independent native landscape vectors on every test run.
use ottd_sim::{Fixture, simulate};
use serde::Deserialize;

#[derive(Deserialize)]
struct Oracle {
    landscape_cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    before: Fixture,
    checkpoints: Vec<Checkpoint>,
}
#[derive(Deserialize)]
struct Checkpoint {
    ticks: u32,
    after: Fixture,
}

#[test]
fn original_tile_procedures_match_all_checkpoints() -> Result<(), Box<dyn std::error::Error>> {
    let oracle: Oracle = serde_json::from_str(include_str!("../../../reference/gameplay.json"))?;
    assert_eq!(oracle.landscape_cases.len(), 4);
    for case in oracle.landscape_cases {
        assert!(!case.checkpoints.is_empty());
        for checkpoint in case.checkpoints {
            assert_eq!(
                simulate(case.before.clone(), checkpoint.ticks)?,
                checkpoint.after,
                "{} at {}",
                case.name,
                checkpoint.ticks
            );
        }
    }
    Ok(())
}
