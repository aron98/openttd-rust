//! Source-bound original company naming prerequisite for populated-world consumers.
use ottd_save::{Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use std::path::{Path, PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

fn company_number(world: &serde_json::Value, name: &str) -> Result<u64> {
    world
        .pointer(&format!("/chunks/PLYR/records/0/{name}"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "missing native company number".into())
}

fn continuation(input: &Path, label: &str) -> Result {
    // Given: the unchanged historical input and a fresh original one-call continuation.
    let native = PathBuf::from(std::env::var("OTTD_COMPANY_NAME_CURRENT")?).join(label);
    let output = PathBuf::from(std::env::var("OTTD_COMPANY_NAME_OUTPUT")?).join(label);
    std::fs::create_dir_all(&output)?;
    let mut runtime = SimulationRuntime::restore_vanilla(load(input)?)?;
    let expected_initial: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("initial.world.json"))?)?;
    assert_eq!(runtime.saved_json()?, expected_initial);
    // When: the composed naming consumer executes at the actual per-company tick.
    runtime.advance_world(1)?;
    // Then: every saved field, derived index and observed live cache equals the original.
    for (view, actual) in [
        ("world", runtime.saved_json()?),
        ("derived", serde_json::to_value(runtime.world().derived())?),
    ] {
        let name = format!("final.{view}.json");
        std::fs::write(output.join(&name), serde_json::to_vec(&actual)?)?;
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(native.join(name))?)?;
        assert_eq!(actual, expected, "{label}/{view}");
    }
    let observed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("native-last-tick.json"))?)?;
    for (view, actual) in [
        (
            "house_caches",
            serde_json::to_value(runtime.town_house_caches())?,
        ),
        (
            "industry_caches",
            serde_json::to_value(runtime.industry_caches())?,
        ),
        ("transport", runtime.transport_state_json()),
        (
            "runtime_road/road",
            serde_json::to_value(runtime.road_caches().values().collect::<Vec<_>>())?,
        ),
    ] {
        let expected = observed
            .pointer(&format!("/movement/{view}"))
            .ok_or("native cache")?;
        assert_eq!(&actual, expected, "{label}/{view}");
        std::fs::write(
            output.join(format!("{}.json", view.replace('/', "-"))),
            serde_json::to_vec(&actual)?,
        )?;
    }
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec(&serde_json::json!({
            "calls":1,"complete_world_derived_rng_and_live_caches":true,"case":label,
        }))?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original-command house naming inputs and fresh original continuations"]
fn original_inert_name_passes_and_active_generation_remains_explicit() -> Result {
    // Preserve the published selector; the original active profile now has a real consumer.
    let directory = PathBuf::from(std::env::var("OTTD_HOUSE_PROOF_DIR")?);
    let inert = directory.join("native-red-256/initial.sav");
    let before = load(&inert)?.saved_json()?;
    assert_eq!(company_number(&before, "name_1")?, 24577);
    assert_eq!(company_number(&before, "last_build_coordinate")?, 0);
    continuation(&inert, "house-inert")?;
    let active = directory.join("native-active-name/active-name.sav");
    let before = load(&active)?.saved_json()?;
    assert_eq!(company_number(&before, "name_1")?, 24577);
    assert_eq!(company_number(&before, "last_build_coordinate")?, 1000);
    continuation(&active, "house-active")
}

#[test]
#[ignore = "requires authentic industry naming inputs and fresh original continuations"]
fn generated_name_matches_original_and_unnamed_active_state_rolls_back() -> Result {
    // Preserve the published selector and replace its stale refusal with native parity.
    let directory = PathBuf::from(std::env::var("OTTD_INDUSTRY_DESIGN_DIR")?);
    let generated = directory.join("generated-name-one/initial.sav");
    let before = load(&generated)?.saved_json()?;
    assert_eq!(company_number(&before, "name_1")?, 28902);
    assert_eq!(company_number(&before, "last_build_coordinate")?, 331);
    continuation(&generated, "industry-generated")?;
    let active = directory.join("manager-search/random-plant-7/final.sav");
    let before = load(&active)?.saved_json()?;
    assert_eq!(company_number(&before, "name_1")?, 0x6001);
    assert_eq!(company_number(&before, "last_build_coordinate")?, 331);
    continuation(&active, "industry-active")
}
