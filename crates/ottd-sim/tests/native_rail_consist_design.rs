//! Success-expecting consist restoration REDs, without changing the single-engine API.
pub mod rail_consist_support;
use ottd_sim::runtime::rail;
use rail_consist_support::{Result, load, native, observation, save, world_equal};
use serde_json::{Value, json};
use std::path::PathBuf;

fn evidence() -> Result<PathBuf> {
    Ok(PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?))
}

#[test]
#[ignore = "requires fresh authentic consist fixture and original executable"]
fn native_both_save_origins_and_observer_neutrality() -> Result {
    let root = evidence()?;
    let input = root.join("prepare/save/autosave/exit.sav");
    let original = load(&input)?;
    let expected = observation(&root.join("observe"))?;
    let rows = expected
        .get("rail")
        .and_then(Value::as_array)
        .ok_or("rail records")?;
    assert!(rows.len() > 1);
    assert_eq!(
        original
            .tables()
            .get(b"VEHS")
            .ok_or("VEHS")?
            .records()
            .len(),
        rows.len()
    );
    assert!(
        rows.iter()
            .all(|v| v.get("stored_count") == Some(&json!(0)))
    );
    let saved = original.saved_json()?;
    for part in rows {
        let id = part.get("id").and_then(Value::as_u64).ok_or("part id")?;
        assert_eq!(
            saved.pointer(&format!("/chunks/VEHS/records/{id}/train/0/gv_flags")),
            Some(&json!(0)),
            "flat terrain alone does not establish saved slope flags"
        );
    }
    let reloaded = load(&root.join("observe/save/autosave/exit.sav"))?;
    world_equal(&original, &reloaded, &root.join("original-roundtrip"))?;
    let rust_save = root.join("rust-origin.sav");
    save(&original, &rust_save)?;
    native(&root.join("rust-origin"), &rust_save, false)?;
    assert_eq!(observation(&root.join("rust-origin"))?, expected);
    world_equal(
        &load(&root.join("rust-origin/save/autosave/exit.sav"))?,
        &reloaded,
        &root.join("rust-origin-roundtrip"),
    )?;
    native(&root.join("observer-disabled"), &input, true)?;
    assert_eq!(
        std::fs::read(root.join("observer-disabled/save/autosave/exit.sav"))?,
        std::fs::read(root.join("observe/save/autosave/exit.sav"))?
    );
    std::fs::write(
        root.join("native-control.txt"),
        "PASS complete cache/RNG equality across original and Rust-serialized load origins; whole saved/derived World equality; observer-disabled exact native save bytes\n",
    )?;
    Ok(())
}

fn restoration_red(input: &std::path::Path, label: &str) -> Result {
    let root = evidence()?;
    let mut world = load(input)?;
    let before = world.saved_json()?;
    let expected = observation(&root.join("observe"))?;
    // The current single-locomotive boundary is deliberately called unchanged. A new
    // consist API is a design proposal, not a stub or an unapproved implementation.
    let actual = rail::restore(&mut world);
    std::fs::write(
        root.join(format!("{label}.result.txt")),
        format!("{actual:?}\n"),
    )?;
    assert!(
        actual.is_ok(),
        "missing consist restoration boundary: {actual:?}"
    );
    let cache = serde_json::to_value(actual?)?;
    let keys = cache.as_object().ok_or("cache object")?;
    let expected_caches = expected
        .get("rail")
        .and_then(Value::as_array)
        .ok_or("rail array")?
        .iter()
        .map(|part| {
            keys.keys()
                .map(|key| {
                    Ok((
                        key.clone(),
                        part.get(key).ok_or("missing native cache")?.clone(),
                    ))
                })
                .collect::<Result<serde_json::Map<String, Value>>>()
                .map(Value::Object)
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(
        vec![cache],
        expected_caches,
        "complete per-part train caches"
    );
    assert_eq!(before, world.saved_json()?);
    world_equal(
        &world,
        &load(&root.join("observe/save/autosave/exit.sav"))?,
        &root.join(format!("{label}.world")),
    )?;
    Ok(())
}

#[test]
#[ignore = "intentional missing-boundary RED using authentic native origin"]
fn original_origin_consist_restoration() -> Result {
    restoration_red(
        &evidence()?.join("prepare/save/autosave/exit.sav"),
        "original-red",
    )
}

#[test]
#[ignore = "intentional missing-boundary RED using Rust serialized origin"]
fn rust_origin_consist_restoration() -> Result {
    restoration_red(&evidence()?.join("rust-origin.sav"), "rust-red")
}
