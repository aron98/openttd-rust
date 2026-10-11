//! Native invocation and complete restoration comparisons shared by rail scenarios.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::rail::{self, RailVehicleCache};
use ottd_sim::runtime::{RuntimeError, SimulationRuntime};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Fallible native evidence operation.
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Decode a real saved world.
/// # Errors
/// Propagates I/O and save decoding failures.
pub fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

/// Invoke the pinned original observer and retain raw process receipts.
/// # Errors
/// Propagates missing environment, I/O and process launch failures.
/// # Panics
/// Fails the scenario if the original executable exits unsuccessfully.
pub fn native(directory: &Path, input: &Path, passive_control: bool) -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let args = vec![
        format!("-DORACLE={}", std::env::var("OTTD_RAIL_ORACLE")?),
        format!("-DRUN_DIR={}", directory.display()),
        format!("-DINPUT={}", input.display()),
        format!("-DPASSIVE_CONTROL={passive_control}"),
        format!("-DCONFIG={}", root.join("scripts/reference.cfg").display()),
        "-P".into(),
        root.join("scripts/check-rail-restore-reference.cmake")
            .display()
            .to_string(),
    ];
    std::fs::write(
        directory.with_extension("argv.json"),
        serde_json::to_vec(&args)?,
    )?;
    let output = Command::new("cmake").args(&args).output()?;
    std::fs::write(directory.with_extension("stdout.log"), &output.stdout)?;
    std::fs::write(directory.with_extension("stderr.log"), &output.stderr)?;
    std::fs::write(
        directory.with_extension("exit.txt"),
        output.status.to_string(),
    )?;
    assert!(
        output.status.success(),
        "native load failed: {}",
        directory.display()
    );
    Ok(())
}

fn snapshot(world: &World, cache: &RailVehicleCache) -> Result<Value> {
    let saved = world.saved_json()?;
    let train = saved
        .pointer(&format!("/chunks/VEHS/records/{}/train/0", cache.id.raw()))
        .ok_or("saved train")?;
    let common = train.pointer("/common/0").ok_or("saved common")?;
    let Value::Object(mut actual) = serde_json::to_value(cache)? else {
        return Err("cache object".into());
    };
    for (key, field) in [
        ("engine_id", "engine_type"),
        ("tile", "tile"),
        ("subtype", "subtype"),
        ("capacity", "cargo_cap"),
        ("current_speed", "cur_speed"),
        ("acceleration", "acceleration"),
        ("vehstatus", "vehstatus"),
    ] {
        actual.insert(key.into(), required(common, field)?.clone());
    }
    actual.insert("flags".into(), required(train, "flags")?.clone());
    let stored = required(common, "cargo.action_counts")?
        .as_array()
        .ok_or("cargo actions")?
        .iter()
        .take(3)
        .map(|count| count.as_u64().ok_or("cargo action count"))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .iter()
        .sum::<u64>();
    actual.insert("stored_count".into(), json!(stored));
    let date = saved
        .pointer("/chunks/DATE/records/0")
        .ok_or("saved date")?;
    Ok(json!({"phase": "after-load-before-tick", "rail": [actual],
        "random": [required(date, "random_state[0]")?, required(date, "random_state[1]")?], "tick": required(date, "tick_counter")?}))
}

fn required<'a>(object: &'a Value, key: &str) -> Result<&'a Value> {
    object
        .get(key)
        .ok_or_else(|| format!("missing field {key}").into())
}

/// Compare complete native caches, saved state and both save-origin continuations.
/// # Errors
/// Propagates native invocation, state decoding, serialization and evidence I/O failures.
/// # Panics
/// Fails the scenario when any native/Rust state comparison differs.
pub fn verify(evidence: &Path, input: &Path) -> Result {
    let expected: Value =
        serde_json::from_slice(&std::fs::read(evidence.join("observe/rail.json"))?)?;
    assert_eq!(required(&expected, "phase")?, "after-load-before-tick");
    let rails = required(&expected, "rail")?
        .as_array()
        .ok_or("native rail array")?;
    assert_eq!(rails.len(), 1);
    let first = rails.first().ok_or("native train")?;
    assert_eq!(required(first, "stored_count")?, 0);
    assert_eq!(required(first, "current_speed")?, 0);
    let mut world = load(input)?;
    assert_eq!(
        world
            .tables()
            .get(b"VEHS")
            .ok_or("saved vehicle table")?
            .records()
            .len(),
        1
    );
    std::fs::write(
        evidence.join("rust.before.json"),
        serde_json::to_vec(&world.saved_json()?)?,
    )?;

    let cache = rail::restore(&mut world)?;
    assert!(matches!(
        SimulationRuntime::restore_vanilla(world.clone()),
        Err(RuntimeError::Unsupported("non-road vehicle"))
    ));

    let actual = snapshot(&world, &cache)?;
    let saved = world.saved_json()?;
    std::fs::write(
        evidence.join("rust.rail.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    assert_eq!(actual, expected);
    std::fs::write(
        evidence.join("rust.after.json"),
        serde_json::to_vec(&saved)?,
    )?;
    let mut native_after = load(&evidence.join("observe/save/autosave/exit.sav"))?;
    let native_saved = native_after.saved_json()?;
    std::fs::write(
        evidence.join("native.after.json"),
        serde_json::to_vec(&native_saved)?,
    )?;
    assert!(
        saved == native_saved,
        "whole saved World differs; inspect retained JSON pair"
    );
    let derived = serde_json::to_value(world.derived())?;
    assert_eq!(derived, serde_json::to_value(native_after.derived())?);
    std::fs::write(
        evidence.join("structural.json"),
        serde_json::to_vec(&derived)?,
    )?;
    let rust_save = evidence.join("rust.sav");
    std::fs::write(&rust_save, world.to_savegame()?.encode(Compression::None)?)?;
    native(&evidence.join("rust-continuation"), &rust_save, false)?;
    let continuation: Value = serde_json::from_slice(&std::fs::read(
        evidence.join("rust-continuation/rail.json"),
    )?)?;
    assert_eq!(continuation, expected);
    assert!(
        load(&evidence.join("rust-continuation/save/autosave/exit.sav"))?.saved_json()? == saved
    );
    let continued_cache = rail::restore(&mut native_after)?;
    assert_eq!(snapshot(&native_after, &continued_cache)?, expected);
    assert!(native_after.saved_json()? == saved);
    std::fs::write(
        evidence.join("native-origin.rust.json"),
        serde_json::to_vec(&snapshot(&native_after, &continued_cache)?)?,
    )?;
    std::fs::write(
        evidence.join("comparison.txt"),
        "PASS complete rail cache, whole saved World, structural projections, tick/RNG, both save-origin load continuations; road runtime still refuses trains\n",
    )?;
    Ok(())
}
