//! Explicit native-world comparison driver; requires fresh external artifacts.
#![cfg(test)]
use ottd_save::{Savegame, world::World};

#[test]
#[ignore = "run through native world driver with WORLD_INPUT/WORLD_EXPECTED/WORLD_OUTPUT"]
fn compares_complete_native_saved_state() -> Result<(), Box<dyn std::error::Error>> {
    // Given an independently native-exported world and its same-checkpoint save.
    let input = std::fs::read(std::env::var("WORLD_INPUT")?)?;
    let save = Savegame::decode(&input, ottd_save::DEFAULT_MAX_BYTES)?;
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var("WORLD_EXPECTED")?)?)?;
    // When Rust restores and exports the world.
    let world = World::decode(&save)?;
    let actual = world.saved_json()?;
    std::fs::write(
        std::env::var("WORLD_OUTPUT")?,
        serde_json::to_vec_pretty(&actual)?,
    )?;
    if let Ok(path) = std::env::var("WORLD_DERIVED_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(world.derived())?)?;
    }
    // Then every persisted field matches the independent native observation.
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
#[ignore = "run through native world driver with WORLD_INPUT/WORLD_DERIVED_EXPECTED/WORLD_DERIVED_OUTPUT"]
fn compares_native_structural_state() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::fs::read(std::env::var("WORLD_INPUT")?)?;
    let save = Savegame::decode(&input, ottd_save::DEFAULT_MAX_BYTES)?;
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var("WORLD_DERIVED_EXPECTED")?)?)?;
    let world = World::decode(&save)?;
    let actual = serde_json::to_value(world.derived())?;
    std::fs::write(
        std::env::var("WORLD_DERIVED_OUTPUT")?,
        serde_json::to_vec_pretty(&actual)?,
    )?;
    assert_eq!(actual, expected);
    Ok(())
}
