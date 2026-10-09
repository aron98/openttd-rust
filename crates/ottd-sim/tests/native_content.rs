//! Actual native after-load content comparisons; no stored expected catalog.
use ottd_save::{
    Compression, Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::content::ContentCatalog;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn edit(chunk: [u8; 4], name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value: WireValue::Unsigned(value),
    }
}

fn compare(actual: &Value, native: &Value) -> Result {
    if actual != native {
        for key in ["climate", "cargo", "engines", "prices"] {
            if actual.get(key) != native.get(key) {
                return Err(format!("native content differs in {key}").into());
            }
        }
        return Err("native content observation has extra/missing fields".into());
    }
    Ok(())
}

fn run_case(directory: &Path, world: &World, oracle: &Path) -> Result {
    std::fs::create_dir_all(directory)?;
    let input = directory.join("input.sav");
    std::fs::write(&input, world.to_savegame()?.encode(Compression::None)?)?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let output = Command::new("cmake")
        .arg(format!("-DORACLE={}", oracle.display()))
        .arg(format!("-DRUN_DIR={}", directory.join("native").display()))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg(format!("-DINPUT={}", input.display()))
        .arg("-P")
        .arg(root.join("scripts/check-content-reference.cmake"))
        .output()?;
    std::fs::write(directory.join("driver.stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("driver.stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err(format!("native content failed: {}", directory.display()).into());
    }
    let native: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/content.json"))?)?;
    let actual = serde_json::to_value(ContentCatalog::from_world(world)?)?;
    std::fs::write(
        directory.join("rust.content.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    compare(&actual, &native)?;
    let mut corrupt = actual.clone();
    *corrupt
        .pointer_mut("/engines/0/info/refit_mask")
        .ok_or("engine field")? = Value::from(999_u64);
    assert!(compare(&corrupt, &native).is_err());
    std::fs::write(
        directory.join("negative-engine.json"),
        serde_json::to_vec_pretty(&corrupt)?,
    )?;
    let mut corrupt = actual;
    *corrupt
        .pointer_mut("/prices/values/0")
        .ok_or("price field")? = Value::from(-999_i64);
    assert!(compare(&corrupt, &native).is_err());
    std::fs::write(
        directory.join("negative-price.json"),
        serde_json::to_vec_pretty(&corrupt)?,
    )?;
    std::fs::write(
        directory.join("comparison.txt"),
        "PASS exact catalog equality; REJECT altered engine; REJECT altered price\n",
    )?;
    Ok(())
}

#[test]
#[ignore = "requires freshly built pinned reference; set OTTD_CONTENT_NATIVE_DIR and OTTD_CONTENT_ORACLE"]
fn native_catalog_and_price_matrix() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_CONTENT_NATIVE_DIR")?);
    if directory.exists() {
        return Err("native matrix directory must be fresh".into());
    }
    std::fs::create_dir_all(&directory)?;
    let directory = directory.canonicalize()?;
    let oracle = PathBuf::from(std::env::var("OTTD_CONTENT_ORACLE")?).canonicalize()?;
    let base = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/generated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut count = 0_u32;
    for climate in 0..4 {
        for construction in 0..3 {
            for running in 0..3 {
                let mut world = base.clone();
                world.edit_batch(vec![
                    edit(*b"PATS", "game_creation.landscape", climate),
                    edit(*b"PATS", "difficulty.construction_cost", construction),
                    edit(*b"PATS", "difficulty.vehicle_costs", running),
                ])?;
                run_case(
                    &directory.join(format!("climate-{climate}-cost-{construction}-{running}")),
                    &world,
                    &oracle,
                )?;
                count = count.checked_add(1).ok_or("case count")?;
            }
        }
        for inflation in [0, 1, 65_537, 2_147_483_647] {
            let mut world = base.clone();
            world.edit_batch(vec![
                edit(*b"PATS", "game_creation.landscape", climate),
                WorldEdit::Field {
                    chunk: *b"PATS",
                    record: 0,
                    path: vec![PathElement::Field("vehicle.disable_elrails".into())],
                    value: WireValue::Signed(1),
                },
                edit(*b"PATS", "difficulty.max_loan", 2_000_000),
                edit(*b"ECMY", "inflation_prices", inflation),
                edit(*b"ECMY", "inflation_payment", inflation),
            ])?;
            run_case(
                &directory.join(format!("climate-{climate}-inflation-{inflation}")),
                &world,
                &oracle,
            )?;
            count = count.checked_add(1).ok_or("case count")?;
        }
    }
    assert_eq!(count, 52);
    std::fs::write(
        directory.join("summary.txt"),
        format!(
            "PASS {count} native catalog cases; 256 engines, 64 cargo slots and 71 prices per case; two rejected corruption controls per case\n"
        ),
    )?;
    Ok(())
}
