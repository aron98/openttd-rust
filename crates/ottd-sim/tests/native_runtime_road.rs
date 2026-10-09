//! Fresh native road cache matrix and exact saved-state preservation.
use ottd_save::{
    Compression, Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::runtime::{SimulationRuntime, VehicleId};
use serde_json::{Value, json};
pub mod road_coverage;
use std::{
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
fn settings(name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"PATS",
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value: WireValue::Unsigned(value),
    }
}

fn native(directory: &Path, input: &Path, prepare: bool) -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let output = Command::new("cmake")
        .arg(format!(
            "-DORACLE={}",
            std::env::var("OTTD_RUNTIME_ORACLE")?
        ))
        .arg(format!("-DRUN_DIR={}", directory.display()))
        .arg(format!("-DINPUT={}", input.display()))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg(format!("-DPREPARE={prepare}"))
        .arg("-P")
        .arg(root.join("scripts/check-runtime-road-reference.cmake"))
        .output()?;
    std::fs::write(directory.with_extension("stdout.log"), &output.stdout)?;
    std::fs::write(directory.with_extension("stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err(format!("native road invocation failed: {}", directory.display()).into());
    }
    Ok(())
}

fn observe(runtime: &SimulationRuntime) -> Result<Value> {
    let mut saved = Vec::new();
    for id in runtime.road_caches().keys() {
        let vehicle = runtime.vehicle(*id)?;
        let engine = runtime.engine(vehicle.engine_id()?)?;
        saved.push(json!({"id": id.raw(), "engine_id": vehicle.engine_id()?, "tile": vehicle.tile()?, "cargo_type": vehicle.cargo_type()?, "capacity": vehicle.capacity()?, "stored_count": vehicle.stored_count()?, "current_speed": vehicle.current_speed()?, "reliability": vehicle.reliability()?, "max_age": vehicle.max_age()?, "engine_reliability": engine.reliability()?, "engine_decay": engine.reliability_decay()?, "engine_age": engine.age()?, "engine_company_availability": engine.company_availability()?}));
    }
    Ok(json!({"road": runtime.road_caches().values().collect::<Vec<_>>(), "saved": saved}))
}

fn compare(actual: &Value, expected: &Value) -> Result {
    if actual != expected {
        return Err("native road runtime mismatch".into());
    }
    Ok(())
}

fn check_case(directory: &Path, world: World) -> Result<usize> {
    std::fs::create_dir_all(directory)?;
    let input = directory.join("input.sav");
    let bytes = world.to_savegame()?.encode(Compression::None)?;
    let before = world.saved_json()?;
    std::fs::write(&input, &bytes)?;
    native(&directory.join("native"), &input, false)?;
    let runtime = SimulationRuntime::restore_vanilla(world)?;
    road_coverage::assert_admitted(&runtime, directory)?;
    assert_eq!(
        runtime.world().saved_json()?,
        before,
        "restoration changed saved state/RNG"
    );
    assert_eq!(
        runtime.world().to_savegame()?.encode(Compression::None)?,
        bytes,
        "restoration changed serialized bytes"
    );
    let actual = observe(&runtime)?;
    let expected: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/runtime.json"))?)?;
    std::fs::write(
        directory.join("rust.runtime.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    std::fs::write(
        directory.join("before.world.json"),
        serde_json::to_vec(&before)?,
    )?;
    std::fs::write(
        directory.join("after.world.json"),
        serde_json::to_vec(&runtime.world().saved_json()?)?,
    )?;
    compare(&actual, &expected)?;
    for (name, pointer) in [
        ("weight", "/road/0/weight"),
        ("cargo-aging", "/road/0/cargo_age_period"),
        ("power", "/road/0/power"),
        ("roadtype", "/road/0/road_type"),
    ] {
        let mut wrong = actual.clone();
        *wrong.pointer_mut(pointer).ok_or("negative control field")? = Value::from(u32::MAX);
        assert!(compare(&wrong, &expected).is_err());
        std::fs::write(
            directory.join(format!("negative-{name}.json")),
            serde_json::to_vec(&wrong)?,
        )?;
    }
    let count = runtime.road_caches().len();
    assert!(count >= 6);
    assert!(runtime.road_cache(VehicleId::new(u32::MAX)).is_err());
    std::fs::write(
        directory.join("comparison.txt"),
        format!(
            "PASS {count} native road caches; exact saved JSON/bytes/RNG unchanged; 4 wrong cache controls rejected\n"
        ),
    )?;
    Ok(count)
}

#[test]
#[ignore = "requires fresh pinned runtime observer; set OTTD_RUNTIME_NATIVE_DIR and OTTD_RUNTIME_ORACLE"]
fn road_cache_native_matrix() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_RUNTIME_NATIVE_DIR")?);
    if directory.exists() {
        return Err("runtime evidence directory must be fresh".into());
    }
    std::fs::create_dir_all(&directory)?;
    let directory = directory.canonicalize()?;
    let base = load(&root.join("fixtures/replay/clear-v362.sav"))?;
    let mut observed = 0_usize;
    let mut cases = 0_u8;
    for climate in 0..4 {
        let mut source = base.clone();
        source.edit_batch(vec![settings("game_creation.landscape", climate)])?;
        let source_path = directory.join(format!("climate-{climate}.sav"));
        std::fs::write(
            &source_path,
            source.to_savegame()?.encode(Compression::None)?,
        )?;
        let preparation = directory.join(format!("prepare-{climate}"));
        native(&preparation, &source_path, true)?;
        let prepared = load(&preparation.join("save/autosave/exit.sav"))?;
        road_coverage::assert_admitted(
            &SimulationRuntime::restore_vanilla(prepared.clone())?,
            &preparation,
        )?;
        road_coverage::assert_rejections(&prepared, &preparation)?;
        for model in 0..2 {
            for steepness in [0, 3, 10] {
                let mut world = prepared.clone();
                world.edit_batch(vec![
                    settings("vehicle.roadveh_acceleration_model", model),
                    settings("vehicle.roadveh_slope_steepness", steepness),
                ])?;
                observed = observed
                    .checked_add(check_case(
                        &directory
                            .join(format!("climate-{climate}-model-{model}-slope-{steepness}")),
                        world,
                    )?)
                    .ok_or("vehicle count")?;
                cases = cases.checked_add(1).ok_or("case count")?;
            }
        }
    }
    assert_eq!(cases, 24);
    std::fs::write(
        directory.join("summary.txt"),
        format!(
            "PASS {cases} cases; {observed} native road cache rows and saved views; 96 rejected corruption controls\n"
        ),
    )?;
    Ok(())
}
