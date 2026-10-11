//! Original runtime cargo caches, restored from genuine cargo save boundaries.
use ottd_save::{Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use std::path::PathBuf;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires independently captured original cargo runtime snapshot"]
fn native_cargo_cache_restore() -> Result {
    let source = PathBuf::from(std::env::var("CARGO_INPUT")?);
    let expected = PathBuf::from(std::env::var("CARGO_EXPECTED")?);
    let output = PathBuf::from(std::env::var("CARGO_OUTPUT")?);
    let world = World::decode(&Savegame::decode(
        &std::fs::read(source)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let runtime = SimulationRuntime::restore_vanilla(world)?;
    let actual = runtime
        .transport_state_json()
        .get("cargo")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let expected: serde_json::Value = serde_json::from_slice(&std::fs::read(expected)?)?;
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("actual.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    std::fs::write(
        output.join("expected.json"),
        serde_json::to_vec_pretty(&expected)?,
    )?;
    assert_eq!(
        actual, expected,
        "restored packet pool, list caches and recipient memberships"
    );
    Ok(())
}
