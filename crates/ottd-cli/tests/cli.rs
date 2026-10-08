//! Tests the executable's exit status, generated saves, and file preservation.

use std::{fs, process::Command};

use ottd_save::{Compression, Savegame};
use tempfile::tempdir;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ottd"))
}

#[test]
fn help_succeeds_without_input_files() {
    let output = command().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());
}

#[test]
fn inspect_reports_machine_readable_chunk_metadata() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.sav");
    fs::write(&input, b"OTTN\x01\x6a\0\0MAPS\0\0\0\x04\0\0\0\x40\0\0\0\0").unwrap();
    let output = command().arg("inspect").arg(input).output().unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report.get("savegame_version").unwrap(), 362);
    let chunk = report.get("chunks").unwrap().get(0).unwrap();
    assert_eq!(chunk.get("id").unwrap(), "MAPS");
    assert_eq!(chunk.get("body_bytes").unwrap(), 4);
}

#[test]
fn rewrite_produces_a_readable_save() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.sav");
    let output_path = dir.path().join("output.sav");
    let bytes = b"OTTN\x01\x6a\0\0MAPS\0\0\0\x04\0\0\0\x40\0\0\0\0";
    fs::write(&input, bytes).unwrap();
    let result = command()
        .arg("rewrite")
        .arg(&input)
        .arg(&output_path)
        .args(["--compression", "lzma"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let rewritten = fs::read(output_path).unwrap();
    let save = Savegame::decode(&rewritten, 1024).unwrap();
    assert_eq!(save.compression(), Compression::Lzma);
    assert_eq!(save.encode(Compression::None).unwrap(), bytes);
}

#[test]
fn rewrite_does_not_replace_an_existing_file() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.sav");
    let destination = dir.path().join("output.sav");
    fs::write(&input, b"OTTN\x01\x6a\0\0\0\0\0\0").unwrap();
    fs::write(&destination, b"keep this data").unwrap();
    let output = command()
        .arg("rewrite")
        .arg(input)
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(destination).unwrap(), b"keep this data");
}

#[test]
fn malformed_input_does_not_create_output() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("bad.sav");
    let destination = dir.path().join("output.sav");
    fs::write(&input, b"OTTN\x01\x6a\0\0DATA\0\0\0\x20short").unwrap();
    let output = command()
        .arg("rewrite")
        .arg(input)
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!destination.exists());
}

#[test]
fn missing_input_fails_cleanly() {
    let dir = tempdir().unwrap();
    let output = command()
        .arg("inspect")
        .arg(dir.path().join("absent.sav"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}

#[test]
fn compare_reports_exact_integer_and_structural_differences() {
    let dir = tempdir().unwrap();
    let expected = dir.path().join("expected.json");
    let actual = dir.path().join("actual.json");
    for (left, right, path) in [
        (
            r#"{"date":{"tick":9007199254740993}}"#,
            r#"{"date":{"tick":9007199254740992}}"#,
            "$.date.tick",
        ),
        (r#"{"tiles":[1,2]}"#, r#"{"tiles":[1]}"#, "$.tiles[1]"),
        (r#"{}"#, r#"{"unknown":1}"#, "$.unknown"),
        (r#"{"missing":null}"#, r#"{}"#, "$.missing"),
    ] {
        fs::write(&expected, left).unwrap();
        fs::write(&actual, right).unwrap();
        let result = command()
            .arg("compare")
            .arg(&expected)
            .arg(&actual)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(path),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn compare_accepts_equal_objects_with_different_key_order() {
    let dir = tempdir().unwrap();
    let expected = dir.path().join("expected.json");
    let actual = dir.path().join("actual.json");
    fs::write(
        &expected,
        r#"{"a":18446744073709551615,"b":-9223372036854775808}"#,
    )
    .unwrap();
    fs::write(
        &actual,
        r#"{"b":-9223372036854775808,"a":18446744073709551615}"#,
    )
    .unwrap();
    let result = command()
        .arg("compare")
        .arg(expected)
        .arg(actual)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn compare_rejects_malformed_and_oversized_inputs() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("input.json");
    fs::write(&path, "{").unwrap();
    assert!(
        !command()
            .arg("compare")
            .arg(&path)
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    fs::write(&path, "{\"a\":1234}").unwrap();
    assert!(
        !command()
            .args(["--max-bytes", "4", "compare"])
            .arg(&path)
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
}
