//! Cargo caches publish atomically with existing vehicle and station commands.
use ottd_save::{Savegame, world::World};
use ottd_sim::{CommandRequest, runtime::SimulationRuntime};
use std::path::PathBuf;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires exact original command cargo snapshots"]
fn native_cargo_command_publication() -> Result {
    let input = std::env::var("CARGO_INPUT")?;
    let native = PathBuf::from(std::env::var("CARGO_NATIVE")?);
    let output = PathBuf::from(std::env::var("CARGO_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("plan.json"))?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &std::fs::read(input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    let actions = plan
        .get("actions")
        .and_then(serde_json::Value::as_array)
        .ok_or("actions")?;
    for action in actions {
        if let Some(request) = action.get("request") {
            let request: CommandRequest = serde_json::from_value(request.clone())?;
            let receipt = runtime.execute_command(&request)?;
            assert!(receipt.posted);
        }
        if let Some(label) = action.get("label").and_then(serde_json::Value::as_str) {
            let expected: serde_json::Value = serde_json::from_slice(&std::fs::read(
                native.join(format!("{label}.cargo.json")),
            )?)?;
            let actual = runtime
                .transport_state_json()
                .get("cargo")
                .cloned()
                .ok_or("cargo view")?;
            std::fs::write(
                output.join(format!("{label}.json")),
                serde_json::to_vec_pretty(&actual)?,
            )?;
            assert_eq!(actual, expected, "cargo cache after {label}");
            let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(
                native.join(format!("{label}.world.json")),
            )?)?;
            let derived: serde_json::Value = serde_json::from_slice(&std::fs::read(
                native.join(format!("{label}.derived.json")),
            )?)?;
            std::fs::write(
                output.join(format!("{label}.world.json")),
                serde_json::to_vec(&runtime.saved_json()?)?,
            )?;
            assert_eq!(
                runtime.saved_json()?,
                saved,
                "complete saved World after {label}"
            );
            assert_eq!(
                serde_json::to_value(runtime.world().derived())?,
                derived,
                "complete derived World after {label}"
            );
        }
    }
    Ok(())
}
