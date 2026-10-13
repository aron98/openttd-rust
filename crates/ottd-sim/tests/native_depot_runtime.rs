//! Original depot pool and complete road infrastructure restoration witnesses.
use ottd_save::{
    Savegame, TileRawParts,
    world::{World, WorldEdit},
};
use ottd_sim::runtime::SimulationRuntime;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn native(root: &Path, output: &Path, input: &Path, prepare: bool, vectors: bool) -> Result {
    let process = Command::new("cmake")
        .arg(format!("-DORACLE={}", std::env::var("OTTD_DEPOT_ORACLE")?))
        .arg(format!("-DRUN_DIR={}", output.display()))
        .arg(format!("-DINPUT={}", input.display()))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg(format!("-DPREPARE={prepare}"))
        .arg(format!("-DVECTORS={vectors}"))
        .arg("-P")
        .arg(root.join("scripts/run-depot-runtime-reference.cmake"))
        .output()?;
    std::fs::write(output.with_extension("stdout.log"), process.stdout)?;
    std::fs::write(output.with_extension("stderr.log"), process.stderr)?;
    if !process.status.success() {
        return Err(format!("native depot failed: {}", output.display()).into());
    }
    Ok(())
}
fn snapshot(world: World) -> Result<Value> {
    let before = world.saved_json()?;
    let runtime = SimulationRuntime::restore_vanilla(world)?;
    assert_eq!(runtime.world().saved_json()?, before);
    let road: BTreeMap<_, _> = runtime
        .road_infrastructure()
        .iter()
        .map(|(id, counts)| (id.to_string(), counts.as_slice()))
        .collect();
    Ok(json!({"pool": runtime.depot_pool(), "road": road}))
}
fn compare(actual: &Value, expected: &Value) -> Result {
    if actual != expected {
        return Err("native depot restoration differs".into());
    }
    Ok(())
}
fn tile_parts(value: &Value) -> Result<TileRawParts> {
    let p: [u16; 10] = serde_json::from_value(value.clone())?;
    Ok(TileRawParts {
        tile_type: u8::try_from(p[0])?,
        height: u8::try_from(p[1])?,
        m1: u8::try_from(p[2])?,
        m2: p[3],
        m3: u8::try_from(p[4])?,
        m4: u8::try_from(p[5])?,
        m5: u8::try_from(p[6])?,
        m6: u8::try_from(p[7])?,
        m7: u8::try_from(p[8])?,
        m8: p[9],
    })
}

#[test]
#[ignore = "requires fresh passive original depot observer"]
fn original_depot_runtime_matrix() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let output = PathBuf::from(std::env::var("OTTD_DEPOT_EVIDENCE")?);
    if output.exists() {
        return Err("depot evidence must be fresh".into());
    }
    std::fs::create_dir_all(&output)?;
    native(
        &root,
        &output.join("prepare"),
        &root.join("fixtures/replay/clear-v362.sav"),
        true,
        false,
    )?;
    let source = output.join("prepare/save/autosave/exit.sav");
    native(&root, &output.join("canonical"), &source, false, false)?;
    native(&root, &output.join("loaded"), &source, false, true)?;
    assert_eq!(
        load(&output.join("canonical/save/autosave/exit.sav"))?.saved_json()?,
        load(&output.join("loaded/save/autosave/exit.sav"))?.saved_json()?,
        "counter-only native type vectors leaked into the saved game"
    );
    let expected: Value =
        serde_json::from_slice(&std::fs::read(output.join("loaded/depot-runtime.json"))?)?;
    let runtime = snapshot(load(&source)?)?;
    compare(&runtime, expected.get("runtime").ok_or("runtime")?)?;
    std::fs::write(
        output.join("rust.runtime.json"),
        serde_json::to_vec_pretty(&runtime)?,
    )?;
    let vectors = expected
        .get("counter_vectors")
        .and_then(Value::as_array)
        .ok_or("vectors")?;
    assert_eq!(vectors.len(), 428);
    let mut actual_vectors = Vec::new();
    for vector in vectors {
        let mut world = load(&source)?;
        let index = u32::try_from(vector["index"].as_u64().ok_or("index")?)?;
        world.edit_batch(vec![WorldEdit::Tile {
            index,
            value: tile_parts(&vector["parts"])?.into(),
        }])?;
        let actual = snapshot(world)?;
        compare(&actual, &vector["runtime"])?;
        actual_vectors.push(actual);
    }
    std::fs::write(
        output.join("rust.vectors.json"),
        serde_json::to_vec_pretty(&actual_vectors)?,
    )?;
    for (name, path) in [
        ("pool", "/pool/first_free"),
        ("road", "/road/0/0"),
        ("tram", "/road/1/1"),
        ("high-slot", "/road/0/62"),
    ] {
        let mut altered = expected.get("runtime").ok_or("runtime")?.clone();
        let value = altered.pointer_mut(path).ok_or("control path")?;
        *value = json!(
            value
                .as_u64()
                .ok_or("control number")?
                .checked_add(1)
                .ok_or("overflow")?
        );
        assert!(compare(&runtime, &altered).is_err());
        std::fs::write(
            output.join(format!("rejected-{name}.json")),
            serde_json::to_vec_pretty(&altered)?,
        )?;
    }
    native(
        &root,
        &output.join("reload"),
        &output.join("loaded/save/autosave/exit.sav"),
        false,
        false,
    )?;
    let reloaded: Value =
        serde_json::from_slice(&std::fs::read(output.join("reload/depot-runtime.json"))?)?;
    compare(&runtime, reloaded.get("runtime").ok_or("runtime")?)?;
    std::fs::write(
        output.join("comparison.txt"),
        "PASS canonical depot/road restore; 428 counter-only vectors; 4 rejected controls; native reload\n",
    )?;
    Ok(())
}
