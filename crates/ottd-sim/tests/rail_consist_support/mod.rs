//! File and process boundary for the native consist design witnesses.
use ottd_save::{Compression, Savegame, world::World};
use serde_json::Value;
use std::{path::Path, process::Command};

/// Fallible native evidence operation.
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Decode an original or Rust saved world.
/// # Errors
/// Propagates file and save decoding errors.
pub fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

/// Encode a complete world without compression.
/// # Errors
/// Propagates encoding and file errors.
pub fn save(world: &World, path: &Path) -> Result {
    std::fs::write(path, world.to_savegame()?.encode(Compression::None)?)?;
    Ok(())
}

/// Read the complete native observation for a test comparison.
/// # Errors
/// Propagates file and JSON errors.
pub fn observation(directory: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(
        directory.join("rail.json"),
    )?)?)
}

/// Execute the original and retain command/process receipts.
/// # Errors
/// Propagates missing environment, file and process errors.
/// # Panics
/// Fails the test if the original exits unsuccessfully.
pub fn native(directory: &Path, input: &Path, disabled: bool) -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let args = vec![
        format!("-DORACLE={}", std::env::var("OTTD_CONSIST_ORACLE")?),
        format!("-DRUN_DIR={}", directory.display()),
        format!("-DINPUT={}", input.display()),
        format!("-DPASSIVE_CONTROL={disabled}"),
        format!("-DCONFIG={}", root.join("scripts/reference.cfg").display()),
        "-P".into(),
        root.join("scripts/check-rail-consist-reference.cmake")
            .display()
            .to_string(),
    ];
    std::fs::write(
        directory.with_extension("argv.json"),
        serde_json::to_vec(&args)?,
    )?;
    let output = Command::new("cmake").args(args).output()?;
    std::fs::write(directory.with_extension("stdout.log"), output.stdout)?;
    std::fs::write(directory.with_extension("stderr.log"), output.stderr)?;
    std::fs::write(
        directory.with_extension("exit.txt"),
        output.status.to_string(),
    )?;
    assert!(
        output.status.success(),
        "native consist load failed: {}",
        directory.display()
    );
    Ok(())
}

/// Compare all saved chunks and structural projections, retaining raw states.
/// # Errors
/// Propagates serialization and file errors.
/// # Panics
/// Fails the test when complete worlds differ.
pub fn world_equal(actual: &World, expected: &World, evidence: &Path) -> Result {
    let saved = actual.saved_json()?;
    let native = expected.saved_json()?;
    std::fs::write(
        evidence.with_extension("rust.json"),
        serde_json::to_vec(&saved)?,
    )?;
    std::fs::write(
        evidence.with_extension("native.json"),
        serde_json::to_vec(&native)?,
    )?;
    assert!(
        saved == native,
        "whole saved World differs at {}",
        evidence.display()
    );
    let derived = serde_json::to_value(actual.derived())?;
    std::fs::write(
        evidence.with_extension("derived.json"),
        serde_json::to_vec(&derived)?,
    )?;
    assert_eq!(derived, serde_json::to_value(expected.derived())?);
    Ok(())
}
