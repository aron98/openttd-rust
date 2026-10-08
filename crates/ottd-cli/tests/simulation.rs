//! CLI limits, malformed inputs and real native terrain checkpoints.
use ottd_sim::Fixture;
use serde::Deserialize;
use std::{fs, process::Command};

#[derive(Deserialize)]
struct Oracle {
    landscape_cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    before: Fixture,
    checkpoints: Vec<Checkpoint>,
}
#[derive(Deserialize)]
struct Checkpoint {
    ticks: u32,
    after: Fixture,
}

#[test]
fn input_limits_require_valid_fixtures() -> Result<(), Box<dyn std::error::Error>> {
    let oracle: Oracle = serde_json::from_str(include_str!("../../../reference/gameplay.json"))?;
    let input = &oracle
        .landscape_cases
        .first()
        .ok_or("missing native case")?
        .before;
    let bytes = serde_json::to_vec(input)?;
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("input.json");
    fs::write(&file, &bytes)?;
    for (limit, ticks, success, message) in [
        (bytes.len(), 0, true, ""),
        (
            bytes.len().saturating_sub(1),
            0,
            false,
            "exceeds --max-bytes",
        ),
        (bytes.len(), 1_000_001, false, "per-request limit"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ottd"))
            .arg("simulate-landscape")
            .arg(&file)
            .arg("--max-bytes")
            .arg(limit.to_string())
            .arg("--ticks")
            .arg(ticks.to_string())
            .output()?;
        assert_eq!(output.status.success(), success);
        if success {
            assert_eq!(serde_json::from_slice::<Fixture>(&output.stdout)?, *input);
        } else {
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(message));
        }
    }
    fs::write(&file, b"{invalid}")?;
    let output = Command::new(env!("CARGO_BIN_EXE_ottd"))
        .arg("simulate-landscape")
        .arg(&file)
        .args(["--ticks", "0"])
        .output()?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid simulation fixture JSON"));
    Ok(())
}

#[test]
#[ignore = "requires OTTD_GAMEPLAY_JSON native oracle output and OTTD_SIMULATION_DIR artifact directory"]
fn native_landscape_checkpoints() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::env::var("OTTD_GAMEPLAY_JSON")?;
    let dir = std::path::PathBuf::from(std::env::var("OTTD_SIMULATION_DIR")?);
    fs::create_dir_all(&dir)?;
    let oracle: Oracle = serde_json::from_slice(&fs::read(source)?)?;
    assert!(!oracle.landscape_cases.is_empty());
    for case in oracle.landscape_cases {
        let input = dir.join(format!("{}-input.json", case.name));
        fs::write(&input, serde_json::to_vec(&case.before)?)?;
        assert!(!case.checkpoints.is_empty());
        for checkpoint in case.checkpoints {
            let stem = format!("{}-{}", case.name, checkpoint.ticks);
            let output = Command::new(env!("CARGO_BIN_EXE_ottd"))
                .arg("simulate-landscape")
                .arg(&input)
                .arg("--ticks")
                .arg(checkpoint.ticks.to_string())
                .output()?;
            fs::write(dir.join(format!("{stem}-stderr.log")), &output.stderr)?;
            fs::write(dir.join(format!("{stem}-rust.json")), &output.stdout)?;
            fs::write(
                dir.join(format!("{stem}-native.json")),
                serde_json::to_vec(&checkpoint.after)?,
            )?;
            assert!(
                output.status.success(),
                "{stem}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let actual: Fixture = serde_json::from_slice(&output.stdout)?;
            assert_eq!(actual, checkpoint.after, "{stem}");
            let repeat = Command::new(env!("CARGO_BIN_EXE_ottd"))
                .arg("simulate-landscape")
                .arg(&input)
                .arg("--ticks")
                .arg(checkpoint.ticks.to_string())
                .output()?;
            assert!(repeat.status.success());
            assert_eq!(output.stdout, repeat.stdout, "repeat {stem}");
            fs::write(dir.join(format!("{stem}-repeat.json")), repeat.stdout)?;
        }
    }
    Ok(())
}
