//! Full native/Rust restoration comparison at both save-origin boundaries.
use super::{Result, load, native, observation, save, snapshot::snapshot, world_equal};
use ottd_sim::runtime::{RuntimeError, SimulationRuntime, rail};
use std::path::Path;

/// Restore and compare all caches, topology, saved state and both load origins.
/// # Errors
/// Propagates restoration and evidence I/O failures.
/// # Panics
/// Fails on any complete native/Rust comparison mismatch.
pub fn verify(root: &Path, input: &Path, label: &str) -> Result {
    let expected = observation(&root.join("observe"))?;
    let mut world = load(input)?;
    save(&world, &root.join(format!("{label}.before.sav")))?;
    let result = rail::restore_consists(&mut world);
    std::fs::write(
        root.join(format!("{label}.result.txt")),
        format!("{result:?}\n"),
    )?;
    let caches = result?;
    let actual = snapshot(&world, &caches)?;
    std::fs::write(
        root.join(format!("{label}.rail.json")),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    assert_eq!(
        actual, expected,
        "complete native cache/link/getter/RNG shape"
    );
    let native_after = load(&root.join("observe/save/autosave/exit.sav"))?;
    world_equal(&world, &native_after, &root.join(format!("{label}.world")))?;
    assert!(matches!(
        SimulationRuntime::restore_vanilla(world.clone()),
        Err(RuntimeError::Unsupported("non-road vehicle"))
    ));
    let rust_save = root.join(format!("{label}.restored.sav"));
    save(&world, &rust_save)?;
    let continuation = root.join(format!("{label}.rust-continuation"));
    native(&continuation, &rust_save, false)?;
    assert_eq!(observation(&continuation)?, expected);
    world_equal(
        &load(&continuation.join("save/autosave/exit.sav"))?,
        &world,
        &root.join(format!("{label}.continued")),
    )?;
    let mut native_origin = native_after;
    let native_caches = rail::restore_consists(&mut native_origin)?;
    assert_eq!(snapshot(&native_origin, &native_caches)?, expected);
    world_equal(
        &native_origin,
        &world,
        &root.join(format!("{label}.native-origin")),
    )?;
    std::fs::write(
        root.join(format!("{label}.comparison.txt")),
        "PASS complete per-part cache/topology/derived stopped getter, entire saved and derived World, tick/RNG, both save-origin restorations; SimulationRuntime still refuses rail\n",
    )?;
    Ok(())
}
