//! Opt-in comparisons with immutable native24 captures; never invokes native or writes savegames.
mod boundaries;
mod controls;
mod coupled;
mod io;
mod model;
mod preparation;
mod replay;

use model::{Action, Domain, Manifest, Native, Protocol};
use serde_json::Value;
use std::path::{Path, PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, thiserror::Error)]
#[error("{case}: {component} mismatch")]
struct Mismatch {
    case: String,
    component: &'static str,
}
fn equal(actual: &Value, expected: &Value, case: &str, component: &'static str) -> Result {
    if actual != expected {
        return Err(Mismatch {
            case: case.into(),
            component,
        }
        .into());
    }
    Ok(())
}
fn corpus() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("TREE_NATIVE_CORPUS")?))
}

#[test]
#[ignore = "requires frozen native24 corpus and a fresh TREE_CORPUS_OUTPUT directory"]
fn native_corpus_complete_receipts_worlds_derived_and_traces() -> Result {
    let directory = corpus()?;
    let manifest: Manifest = io::json(&directory.join("draft/native-cases.json"))?;
    assert_eq!(manifest.schema_version, 1);
    let mut count = 0_usize;
    for case in manifest.cases {
        match case.domain {
            Domain::TerrainParity => {
                let result = replay::run(&case.directory, true, &format!("normal-{}", case.id))?;
                io::record(&format!("normal-{}", case.id), &result)?;
                count = count.saturating_add(1);
            }
            Domain::LoaderClampBoundary
            | Domain::EarlyCompanyGate
            | Domain::ObserverOffControl
            | Domain::UnsupportedBridge
            | Domain::UnsupportedDeity
            | Domain::UnsupportedWater => {}
        }
    }
    assert_eq!(count, 45);
    println!("NATIVE_CORPUS_MATCH runs={count}");
    Ok(())
}
