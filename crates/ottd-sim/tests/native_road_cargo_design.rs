//! Success-expecting original cargo witnesses; product remains unimplemented.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn input() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(std::env::var("CARGO_INPUT")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

#[test]
#[ignore = "requires retained command-created original cargo world"]
fn rust_save_origin_export() -> Result {
    let world = input()?;
    let output = PathBuf::from(std::env::var("CARGO_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let encoded = world.encode(Compression::None)?;
    let decoded = World::decode(&Savegame::decode(&encoded, ottd_save::DEFAULT_MAX_BYTES)?)?;
    assert_eq!(
        world.saved_json()?,
        decoded.saved_json()?,
        "Rust save export changed complete saved World"
    );
    assert_eq!(
        serde_json::to_value(world.derived())?,
        serde_json::to_value(decoded.derived())?,
        "Rust save export changed derived World"
    );
    std::fs::write(output.join("final.sav"), encoded)?;
    std::fs::write(
        output.join("result.json"),
        b"{\"complete_world_roundtrip\":true}",
    )?;
    Ok(())
}

#[test]
#[ignore = "requires native positive recipient/loading/delivery evidence"]
fn native_road_cargo_horizon() -> Result {
    let output = PathBuf::from(std::env::var("CARGO_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let ticks = std::env::var("CARGO_TICKS")?.parse::<u32>()?;
    let restored = SimulationRuntime::restore_vanilla(input()?);
    std::fs::write(
        output.join("restore.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "success": restored.is_ok(),
            "error": restored.as_ref().err().map(ToString::to_string),
        }))?,
    )?;
    let mut runtime = restored?;
    let before = format!("{runtime:?}");
    let before_saved = runtime.saved_json()?;
    let result = runtime.advance_world(ticks);
    let unchanged = before == format!("{runtime:?}");
    std::fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ticks": ticks, "success": result.is_ok(),
            "error": result.as_ref().err().map(ToString::to_string),
            "entire_runtime_unchanged": unchanged,
            "saved_world_unchanged": before_saved == runtime.saved_json()?,
        }))?,
    )?;
    if result.is_err() {
        assert!(unchanged, "cargo rejection published candidate state");
    }
    assert!(
        result.is_ok(),
        "native cargo horizon must succeed: {result:?}"
    );
    std::fs::write(
        output.join("final.sav"),
        runtime.to_savegame()?.encode(Compression::None)?,
    )?;
    std::fs::write(
        output.join("final.world.json"),
        serde_json::to_vec(&runtime.saved_json()?)?,
    )?;
    Ok(())
}
