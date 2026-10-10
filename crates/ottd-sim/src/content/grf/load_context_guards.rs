use super::{Result, manifest, matrix};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

enum Guard {
    Field(&'static str, Value, &'static str),
    MissingOutput,
    DuplicateOutput,
    Replay,
    ReusedDirectory,
    NonSave,
    StaleSource,
}

fn stale_source(root: &Path, directory: &Path) -> Result<PathBuf> {
    let target = directory.join("stale-source");
    std::fs::create_dir(&target)?;
    let scripts = target.join("scripts");
    let reference = target.join("reference");
    std::fs::create_dir(&scripts)?;
    std::fs::create_dir(&reference)?;
    for name in [
        "check-replay-build.cmake",
        "check-grf-load-control-reference.cmake",
    ] {
        std::fs::copy(root.join("scripts").join(name), scripts.join(name))?;
    }
    for entry in std::fs::read_dir(root.join("reference"))? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::copy(entry.path(), reference.join(entry.file_name()))?;
        }
    }
    let header = reference.join("grf_load_context.hpp");
    let mut bytes = std::fs::read(&header)?;
    bytes.push(b'\n');
    std::fs::write(header, bytes)?;
    Ok(scripts.join("check-grf-load-control-reference.cmake"))
}

fn run(root: &Path, directory: &Path, case: &matrix::Case, guard: Guard) -> Result {
    std::fs::create_dir(directory)?;
    let mut value = manifest(case, directory)?;
    let context = directory.join("context.json");
    let native = directory.join("native");
    let mut runner = root.join("scripts/check-grf-load-control-reference.cmake");
    let mut input = root
        .join("fixtures/replay/clear-v362.sav")
        .display()
        .to_string();
    let mut command = Command::new("cmake");
    command.env("OTTD_GRF_CONTEXT_OUTPUT", &context);
    let (diagnostic, early) = match guard {
        Guard::Field(pointer, replacement, diagnostic) => {
            *value.pointer_mut(pointer).ok_or("guard field")? = replacement;
            (diagnostic, false)
        }
        Guard::MissingOutput => {
            command.env_remove("OTTD_GRF_CONTEXT_OUTPUT");
            ("missing context output", false)
        }
        Guard::DuplicateOutput => {
            std::fs::write(&context, b"sentinel\n")?;
            ("context output already exists", false)
        }
        Guard::Replay => {
            command.env("OTTD_REPLAY_PATH", directory.join("replay.json"));
            ("requires a dedicated invocation", true)
        }
        Guard::ReusedDirectory => {
            std::fs::create_dir(&native)?;
            ("run directory already exists", true)
        }
        Guard::NonSave => {
            input = "GENERATE".into();
            ("requires an explicit saved-game load", true)
        }
        Guard::StaleSource => {
            runner = stale_source(root, directory)?;
            ("Stale native replay binary/source", true)
        }
    };
    let path = directory.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(&value)?)?;
    let output = command
        .arg(format!(
            "-DORACLE={}",
            std::env::var("OTTD_GRF_CONTEXT_ORACLE")?
        ))
        .arg(format!("-DRUN_DIR={}", native.display()))
        .arg(format!("-DMANIFEST={}", path.display()))
        .arg(format!("-DINPUT={input}"))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg("-P")
        .arg(runner)
        .output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    let message = if early {
        String::from_utf8(output.stderr)?
    } else {
        std::fs::read_to_string(native.join("stderr.log"))?
    };
    if output.status.success() || !message.contains(diagnostic) {
        return Err(format!("guard failed: {diagnostic}").into());
    }
    if native.join("control.json").exists() || native.join("save/autosave/exit.sav").exists() {
        return Err("refused context produced successful output".into());
    }
    if context.exists() && std::fs::read(&context)? != b"sentinel\n" {
        return Err("refused context overwrote output".into());
    }
    std::fs::write(
        directory.join("comparison.txt"),
        format!("PASS rejected via actual surface: {diagnostic}; no successful observation/save\n"),
    )?;
    Ok(())
}

#[test]
#[ignore = "requires the original context observer and fresh artifact directory"]
fn original_context_host_guards() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_CONTEXT_GUARDS")?);
    if !directory.is_absolute() {
        return Err("absolute guard directory required".into());
    }
    std::fs::create_dir(&directory)?;
    let cases = matrix::all()?;
    let case = cases.first().ok_or("fixture")?;
    let guards = [
        (
            "mode",
            Guard::Field("/context/game_mode", json!(4), "invalid fixture enum"),
        ),
        (
            "units",
            Guard::Field(
                "/context/timekeeping_units",
                json!(2),
                "invalid fixture enum",
            ),
        ),
        (
            "climate",
            Guard::Field("/context/climate", json!(4), "invalid fixture enum"),
        ),
        (
            "date",
            Guard::Field(
                "/context/calendar_date",
                json!(-1),
                "invalid fixture date domain",
            ),
        ),
        (
            "year",
            Guard::Field(
                "/context/starting_year",
                json!(-1),
                "invalid fixture date domain",
            ),
        ),
        (
            "map",
            Guard::Field(
                "/context/map_width",
                json!(128),
                "fixture map must come from actual input save",
            ),
        ),
        (
            "palette",
            Guard::Field(
                "/files/0/palette",
                json!(2),
                "invalid selected palette/config",
            ),
        ),
        ("missing-output", Guard::MissingOutput),
        ("duplicate-output", Guard::DuplicateOutput),
        ("replay", Guard::Replay),
        ("reused", Guard::ReusedDirectory),
        ("non-save", Guard::NonSave),
        ("stale", Guard::StaleSource),
    ];
    for (name, guard) in guards {
        run(&root, &directory.join(name), case, guard)?;
    }
    date_guards(&root, &directory, &cases)?;
    println!("PASS 18 actual context host guards");
    Ok(())
}

fn date_guards(root: &Path, directory: &Path, cases: &[matrix::Case]) -> Result {
    let case = cases.first().ok_or("date fixture")?;
    for (name, pointer, raw) in [
        ("year-upper", "/context/starting_year", 5_000_001),
        ("economy-negative", "/context/economy_date", -1),
        ("economy-upper", "/context/economy_date", i32::MAX),
    ] {
        run(
            root,
            &directory.join(name),
            case,
            Guard::Field(pointer, json!(raw), "invalid fixture date domain"),
        )?;
    }
    let wallclock = cases
        .iter()
        .find(|case| case.program.name == "network-Wallclock")
        .ok_or("wallclock fixture")?;
    for (name, raw) in [
        ("wallclock-negative", -1),
        ("wallclock-upper", 1_800_000_360),
    ] {
        run(
            root,
            &directory.join(name),
            wallclock,
            Guard::Field(
                "/context/economy_date",
                json!(raw),
                "invalid fixture date domain",
            ),
        )?;
    }
    Ok(())
}
