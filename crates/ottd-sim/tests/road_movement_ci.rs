//! External original-generated movement corpus capture through public runtime APIs.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde::Deserialize;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    input: PathBuf,
    output: PathBuf,
    calls: u32,
}

fn write(path: &Path, bytes: &[u8]) -> Result {
    let mut output = File::create_new(path)?;
    output.write_all(bytes)?;
    output.sync_all()?;
    Ok(())
}

fn checkpoint(runtime: &SimulationRuntime, directory: &Path, label: &str) -> Result {
    write(
        &directory.join(format!("{label}.world.json")),
        &serde_json::to_vec(&runtime.saved_json()?)?,
    )?;
    write(
        &directory.join(format!("{label}.derived.json")),
        &serde_json::to_vec(runtime.world().derived())?,
    )?;
    write(
        &directory.join(format!("{label}.content.json")),
        &serde_json::to_vec(runtime.content())?,
    )?;
    let caches: Vec<_> = runtime.road_caches().values().collect();
    let physical = serde_json::json!({
        "capability": "single-road-gameplay-physical-no-viewport",
        "road_caches": caches,
        "single_road_tile_occupancy": runtime.single_road_tile_occupancy()?,
    });
    write(
        &directory.join(format!("{label}.physical.json")),
        &serde_json::to_vec(&physical)?,
    )?;
    write(
        &directory.join(format!("{label}.sav")),
        &runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires a fresh original-generated corpus; normal movement driver runs this exact selector"]
fn capture_public_runtime() -> Result {
    let request_path = PathBuf::from(std::env::var("OTTD_MOVEMENT_RUST_REQUEST")?);
    let request: Request = serde_json::from_slice(&std::fs::read(&request_path)?)?;
    if request.calls > 256 || !request.input.is_absolute() || !request.output.is_absolute() {
        return Err("invalid bounded movement capture request".into());
    }
    let world = World::decode(&Savegame::decode(
        &std::fs::read(&request.input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    std::fs::create_dir(&request.output)?;
    checkpoint(&runtime, &request.output, "initial")?;
    for call in 1..=request.calls {
        runtime.advance_world(1)?;
        checkpoint(&runtime, &request.output, &format!("tick_{call}"))?;
    }
    checkpoint(&runtime, &request.output, "final")?;
    let receipt = serde_json::json!({"calls": request.calls, "input": request.input,
        "capability": "single-road-gameplay-physical-no-viewport", "complete": true});
    write(
        &request.output.join("capture.json"),
        &serde_json::to_vec(&receipt)?,
    )?;
    Ok(())
}

#[path = "road_movement_ci/controls.rs"]
mod controls;
