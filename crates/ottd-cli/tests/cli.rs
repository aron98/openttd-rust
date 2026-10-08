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
