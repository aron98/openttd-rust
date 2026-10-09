//! Real-process saved-world inspection and transactional edit checks.
#![cfg(test)]
use serde_json::json;
use std::{fs, path::PathBuf, process::Command};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ottd"))
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/generated-v362.sav")
}
fn change(value: serde_json::Value) -> serde_json::Value {
    let mut edit =
        json!({"kind":"field","chunk":"PATS","record":0,"path":["difficulty.max_no_competitors"]});
    edit.as_object_mut()
        .unwrap()
        .insert("value".to_owned(), value);
    edit
}

#[test]
fn world_exports_saved_and_derived_views() {
    // Given a supported native save, when each CLI view is requested.
    let all = command().arg("world").arg(fixture()).output().unwrap();
    assert!(
        all.status.success(),
        "{}",
        String::from_utf8_lossy(&all.stderr)
    );
    let all: serde_json::Value = serde_json::from_slice(&all.stdout).unwrap();
    for view in ["saved", "derived"] {
        let result = command()
            .arg("world")
            .arg(fixture())
            .args(["--view", view])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        // Then the full envelope contains exactly the corresponding canonical view.
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(all.get(view), Some(&value));
    }
    assert_eq!(
        all.pointer("/saved/chunks/MAPS/records/0/dim_x"),
        Some(&json!(64))
    );
}

#[test]
fn edits_survive_save_and_cli_world_reload() {
    // Given a supported setting mutation and a new destination.
    let dir = tempfile::tempdir().unwrap();
    let edits = dir.path().join("edits.json");
    let output = dir.path().join("edited.sav");
    fs::write(
        &edits,
        json!({"schema_version":1,"edits":[change(json!({"unsigned":3}))]}).to_string(),
    )
    .unwrap();
    // When the edit command writes a native save and the CLI loads it again.
    let result = command()
        .arg("edit-world")
        .arg(fixture())
        .arg(&edits)
        .arg(&output)
        .args(["--compression", "none"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = command()
        .arg("world")
        .arg(&output)
        .args(["--view", "saved"])
        .output()
        .unwrap();
    // Then the exact edited field survives.
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let world: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        world.pointer("/chunks/PATS/records/0/difficulty.max_no_competitors"),
        Some(&json!(3))
    );
}

#[test]
fn rejected_edit_batches_publish_nothing_and_preserve_input() {
    // Given one valid edit followed by a failing shape, path or value.
    let before = fs::read(fixture()).unwrap();
    for invalid in [
        change(json!({"unsigned":256})),
        change(json!({"signed":1})),
        json!({"kind":"field","chunk":"PATS","record":0,"path":["missing"],"value":{"unsigned":1}}),
        json!({"kind":"field","chunk":"PATS","record":99,"path":["difficulty.max_no_competitors"],"value":{"unsigned":1}}),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let edits = dir.path().join("edits.json");
        let output = dir.path().join("output.sav");
        fs::write(
            &edits,
            json!({"schema_version":1,"edits":[change(json!({"unsigned":2})),invalid]}).to_string(),
        )
        .unwrap();
        // When the batch is applied, then no file is published and the source is unchanged.
        let result = command()
            .arg("edit-world")
            .arg(fixture())
            .arg(edits)
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!output.exists());
        assert_eq!(fs::read(fixture()).unwrap(), before);
    }
}

#[test]
fn malformed_documents_and_existing_destinations_are_rejected() {
    // Given malformed or ambiguous JSON documents.
    for document in [
        "{",
        r#"{"schema_version":1,"schema_version":1,"edits":[]}"#,
        r#"{"schema_version":2,"edits":[]}"#,
        r#"{"schema_version":1,"edits":[],"extra":0}"#,
        r#"{"schema_version":1,"edits":[{"kind":"field","chunk":"PATS","record":0,"path":[],"value":{"unsigned":1.0}}]}"#,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let edits = dir.path().join("edits.json");
        let output = dir.path().join("output.sav");
        fs::write(&edits, document).unwrap();
        // When invoked, then failure does not publish an output.
        let result = command()
            .arg("edit-world")
            .arg(fixture())
            .arg(&edits)
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
    }
    let dir = tempfile::tempdir().unwrap();
    let edits = dir.path().join("edits.json");
    let output = dir.path().join("output.sav");
    fs::write(&edits, r#"{"schema_version":1,"edits":[]}"#).unwrap();
    fs::write(&output, b"keep").unwrap();
    let result = command()
        .arg("edit-world")
        .arg(fixture())
        .arg(edits)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read(output).unwrap(), b"keep");
}

#[test]
fn edit_document_size_and_count_are_bounded() {
    // Given excessive document bytes or an excessive number of operations.
    for document in [
        " ".repeat(1_048_577),
        json!({"schema_version":1,"edits":vec![change(json!({"unsigned":2}));1025]}).to_string(),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let edits = dir.path().join("edits.json");
        let output = dir.path().join("output.sav");
        fs::write(&edits, document).unwrap();
        // When the CLI reads the edit file, then it rejects it without a destination.
        let result = command()
            .arg("edit-world")
            .arg(fixture())
            .arg(edits)
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
    }
}

#[test]
fn dangling_vehicle_reference_is_rejected_without_output() {
    // Given the pinned populated world and a nonexistent vehicle reference.
    let input =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/populated-v362.sav");
    let dir = tempfile::tempdir().unwrap();
    let edits = dir.path().join("edits.json");
    let output = dir.path().join("output.sav");
    fs::write(&edits,json!({"schema_version":1,"edits":[{"kind":"field","chunk":"VEHS","record":17,"path":["train",0,"common",0,"next"],"value":{"unsigned":u32::MAX}}]}).to_string()).unwrap();
    // When applied, then graph validation fails before publishing.
    let result = command()
        .arg("edit-world")
        .arg(input)
        .arg(edits)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!output.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("world edit batch failed"));
}

#[test]
fn coupled_order_edits_are_validated_as_one_transaction() {
    // Given two road vehicles whose order list and shared links must change together.
    let input =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/populated-v362.sav");
    let dir = tempfile::tempdir().unwrap();
    let edits = dir.path().join("edits.json");
    let output = dir.path().join("output.sav");
    let link = json!({"kind":"field","chunk":"VEHS","record":12,"path":["roadveh",0,"common",0,"next_shared"],"value":{"unsigned":14}});
    let orders = json!({"kind":"field","chunk":"VEHS","record":13,"path":["roadveh",0,"common",0,"orders"],"value":{"unsigned":1}});
    fs::write(
        &edits,
        json!({"schema_version":1,"edits":[link,orders]}).to_string(),
    )
    .unwrap();
    // When both changes are submitted, then final graph validation permits the batch.
    let result = command()
        .arg("edit-world")
        .arg(input)
        .arg(edits)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = command()
        .arg("world")
        .arg(output)
        .args(["--view", "derived"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let derived: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let orders = derived
        .get("order_lists")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row.get("id") == Some(&json!(0)))
        .unwrap();
    assert_eq!(orders.get("vehicles"), Some(&json!([12, 13])));
}

#[test]
fn raw_tile_edit_survives_native_container_reload() {
    // Given an existing tile and a change to its saved height.
    let bytes = fs::read(fixture()).unwrap();
    let snapshot = ottd_save::Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)
        .unwrap()
        .snapshot()
        .unwrap();
    let mut tile = serde_json::to_value(snapshot.map().tiles().get(100).unwrap()).unwrap();
    let height = tile.get("height").unwrap().as_u64().unwrap();
    tile.as_object_mut()
        .unwrap()
        .insert("height".to_owned(), json!(height ^ 1));
    let dir = tempfile::tempdir().unwrap();
    let edits = dir.path().join("edits.json");
    let output = dir.path().join("output.sav");
    fs::write(
        &edits,
        json!({"schema_version":1,"edits":[{"kind":"tile","index":100,"value":tile}]}).to_string(),
    )
    .unwrap();
    // When the CLI publishes the edited save, then reloading preserves the complete tile.
    let result = command()
        .arg("edit-world")
        .arg(fixture())
        .arg(edits)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let saved = fs::read(output).unwrap();
    let snapshot = ottd_save::Savegame::decode(&saved, ottd_save::DEFAULT_MAX_BYTES)
        .unwrap()
        .snapshot()
        .unwrap();
    assert_eq!(
        serde_json::to_value(snapshot.map().tiles().get(100).unwrap()).unwrap(),
        tile
    );
}
