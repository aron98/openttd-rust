//! Real-process replay, resume and no-clobber publication checks.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/replay/populated-v362.sav")
}
fn invoke(args: &[&Path]) -> Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_ottd"))
        .args(args)
        .output()?)
}
fn succeeds(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn plan() -> Value {
    json!({"schema_version":1,"actions":[
        {"ordinal":0,"op":"command","request":{"company":0,"mode":"estimate","command":{"kind":"build_road","tile":3184,"pieces":5,"road_type":0,"toggle_disallowed":0,"town_id":65535}}},
        {"ordinal":1,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"build_road","tile":3184,"pieces":5,"road_type":0,"toggle_disallowed":0,"town_id":65535}}},
        {"ordinal":2,"op":"checkpoint","label":"built"},
        {"ordinal":3,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"build_road","tile":3184,"pieces":5,"road_type":0,"toggle_disallowed":0,"town_id":65535}}}
    ]})
}
#[test]
fn road_replay_and_resume_match_continuous_saved_state() -> Result {
    let dir = tempfile::tempdir()?;
    let actions = dir.path().join("actions.json");
    fs::write(&actions, serde_json::to_vec(&plan())?)?;
    let continuous = dir.path().join("continuous");
    succeeds(&invoke(&[
        Path::new("replay-world"),
        &fixture(),
        &actions,
        &continuous,
    ])?);
    let split = dir.path().join("split");
    succeeds(&invoke(&[
        Path::new("replay-world"),
        &fixture(),
        &actions,
        &split,
        Path::new("--through"),
        Path::new("1"),
    ])?);
    let resumed = dir.path().join("resumed");
    succeeds(&invoke(&[
        Path::new("resume-world"),
        &split.join("checkpoint.json"),
        &resumed,
    ])?);
    assert_eq!(
        fs::read(continuous.join("final.world.json"))?,
        fs::read(resumed.join("final.world.json"))?
    );
    assert!(resumed.join("built.sav").is_file());
    let results: Value = serde_json::from_slice(&fs::read(continuous.join("results.json"))?)?;
    assert_eq!(
        results.pointer("/actions/0/receipt/exec"),
        Some(&Value::Null)
    );
    assert_eq!(
        results.pointer("/actions/3/receipt/result/error"),
        Some(&json!("STR_ERROR_ALREADY_BUILT"))
    );
    Ok(())
}
#[test]
fn malformed_or_unsupported_replay_publishes_no_directory() -> Result {
    for document in [
        r#"{"schema_version":1,"schema_version":1,"actions":[]}"#,
        r#"{"schema_version":1,"actions":[{"ordinal":1.0,"op":"tick","count":0}]}"#,
        r#"{"schema_version":1,"actions":[{"ordinal":0,"op":"checkpoint","label":"../escape"}]}"#,
        r#"{"schema_version":1,"actions":[{"ordinal":0,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"pause","mode":0,"paused":false}}},{"ordinal":1,"op":"tick","count":1}]}"#,
    ] {
        let dir = tempfile::tempdir()?;
        let actions = dir.path().join("actions.json");
        fs::write(&actions, document)?;
        let output = dir.path().join("output");
        let result = invoke(&[Path::new("replay-world"), &fixture(), &actions, &output])?;
        assert!(!result.status.success());
        assert!(!output.exists());
    }
    Ok(())
}
#[test]
fn existing_output_is_preserved() -> Result {
    let dir = tempfile::tempdir()?;
    let actions = dir.path().join("actions.json");
    fs::write(&actions, serde_json::to_vec(&plan())?)?;
    let output = dir.path().join("output");
    fs::create_dir(&output)?;
    fs::write(output.join("sentinel"), b"keep")?;
    let result = invoke(&[Path::new("replay-world"), &fixture(), &actions, &output])?;
    assert!(!result.status.success());
    assert_eq!(fs::read(output.join("sentinel"))?, b"keep");
    assert_eq!(fs::read_dir(&output)?.count(), 1);
    Ok(())
}

#[test]
fn resume_rejects_changed_save_hash_and_unsafe_reference() -> Result {
    let dir = tempfile::tempdir()?;
    let actions = dir.path().join("actions.json");
    fs::write(&actions, serde_json::to_vec(&plan())?)?;
    let prefix = dir.path().join("prefix");
    succeeds(&invoke(&[
        Path::new("replay-world"),
        &fixture(),
        &actions,
        &prefix,
        Path::new("--through"),
        Path::new("1"),
    ])?);
    let original: Value = serde_json::from_slice(&fs::read(prefix.join("checkpoint.json"))?)?;
    for (name, pointer, value) in [
        ("hash", "/save/sha256", json!("0".repeat(64))),
        ("path", "/save/file", json!("../outside.sav")),
        ("position", "/cursor/position", json!(999)),
    ] {
        let mut envelope = original.clone();
        *envelope.pointer_mut(pointer).ok_or("checkpoint field")? = value;
        let checkpoint = prefix.join(format!("{name}.json"));
        fs::write(&checkpoint, serde_json::to_vec(&envelope)?)?;
        let output = dir.path().join(name);
        let result = invoke(&[Path::new("resume-world"), &checkpoint, &output])?;
        assert!(!result.status.success(), "{name}");
        assert!(!output.exists(), "{name}");
    }
    let save = prefix.join("final.sav");
    let mut bytes = fs::read(&save)?;
    bytes.push(0);
    fs::write(save, bytes)?;
    let output = dir.path().join("modified-save");
    let result = invoke(&[
        Path::new("resume-world"),
        &prefix.join("checkpoint.json"),
        &output,
    ])?;
    assert!(!result.status.success());
    assert!(!output.exists());
    Ok(())
}

#[test]
fn concurrent_publications_allow_exactly_one_owner() -> Result {
    let dir = tempfile::tempdir()?;
    let actions = dir.path().join("actions.json");
    fs::write(&actions, serde_json::to_vec(&plan())?)?;
    let output = dir.path().join("output");
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_ottd"))
            .arg("replay-world")
            .arg(fixture())
            .arg(&actions)
            .arg(&output)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    };
    let first = spawn()?;
    let second = spawn()?;
    let first = first.wait_with_output()?;
    let second = second.wait_with_output()?;
    assert_ne!(first.status.success(), second.status.success());
    assert!(output.join("results.json").is_file() && output.join("checkpoint.json").is_file());
    let reopened = invoke(&[Path::new("world"), &output.join("final.sav")])?;
    succeeds(&reopened);
    Ok(())
}

#[test]
fn rejects_save_whose_native_load_would_create_a_company() -> Result {
    let dir = tempfile::tempdir()?;
    let actions = dir.path().join("actions.json");
    fs::write(&actions, br#"{"schema_version":1,"actions":[]}"#)?;
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/generated-v362.sav");
    let output = dir.path().join("output");
    let result = invoke(&[Path::new("replay-world"), &input, &actions, &output])?;
    assert!(!result.status.success());
    assert!(!output.exists());
    Ok(())
}
