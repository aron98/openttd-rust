use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone, Copy)]
enum Guard {
    Replay,
    Subset,
    Reuse,
    NonSave,
    Stale,
    Duplicate,
    MissingInput,
    MissingOutput,
    Unarmed,
}

fn stale_runner(root: &Path, directory: &Path) -> Result<PathBuf> {
    let target = directory.join("stale");
    std::fs::create_dir(&target)?;
    for subdir in ["scripts", "reference"] {
        std::fs::create_dir(target.join(subdir))?;
    }
    for script in [
        "check-replay-build.cmake",
        "check-grf-safety-reference.cmake",
    ] {
        std::fs::copy(
            root.join("scripts").join(script),
            target.join("scripts").join(script),
        )?;
    }
    for entry in std::fs::read_dir(root.join("reference"))? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::copy(
                entry.path(),
                target.join("reference").join(entry.file_name()),
            )?;
        }
    }
    let header = target.join("reference/grf_safety.hpp");
    let mut bytes = std::fs::read(&header)?;
    bytes.push(b'\n');
    std::fs::write(header, bytes)?;
    Ok(target.join("scripts/check-grf-safety-reference.cmake"))
}

fn run(root: &Path, directory: &Path, guard: Guard) -> Result {
    std::fs::create_dir(directory)?;
    let native = directory.join("native");
    let observation = directory.join("safety.json");
    let manifest = directory.join("manifest.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(
            &json!([{"id":"guard","path":root.join("fixtures/content/contract-speed.grf"),"configs":[],"is_static":true}]),
        )?,
    )?;
    let mut input = root
        .join("fixtures/replay/clear-v362.sav")
        .display()
        .to_string();
    let mut runner = root.join("scripts/check-grf-safety-reference.cmake");
    let mut command = Command::new("cmake");
    let (diagnostic, early) = match guard {
        Guard::Replay => {
            command.env("OTTD_REPLAY_PATH", "forbidden");
            ("forbids OTTD_REPLAY_PATH", true)
        }
        Guard::Subset => {
            command.env("OTTD_GRF_SAFETY_CASE", "only-one");
            ("forbids OTTD_GRF_SAFETY_CASE", true)
        }
        Guard::Reuse => {
            std::fs::create_dir(&native)?;
            ("directory already exists", true)
        }
        Guard::NonSave => {
            input = "GENERATE".into();
            ("requires a saved game", true)
        }
        Guard::Stale => {
            runner = stale_runner(root, directory)?;
            ("Stale native replay binary/source", true)
        }
        Guard::Duplicate | Guard::MissingInput | Guard::MissingOutput | Guard::Unarmed => {
            runner = root.join("scripts/run-reference.cmake");
            command
                .env("OTTD_GRF_SAFETY_INPUT", &manifest)
                .env("OTTD_GRF_SAFETY_OUTPUT", &observation);
            match guard {
                Guard::Duplicate => {
                    std::fs::write(&observation, b"sentinel\n")?;
                    ("output already exists", false)
                }
                Guard::MissingInput => {
                    command.env_remove("OTTD_GRF_SAFETY_INPUT");
                    ("dedicated saved-game invocation required", false)
                }
                Guard::MissingOutput => {
                    command.env_remove("OTTD_GRF_SAFETY_OUTPUT");
                    ("dedicated saved-game invocation required", false)
                }
                Guard::Unarmed => {
                    command
                        .env_remove("OTTD_GRF_SAFETY_INPUT")
                        .env_remove("OTTD_GRF_SAFETY_OUTPUT");
                    ("", false)
                }
                Guard::Replay | Guard::Subset | Guard::Reuse | Guard::NonSave | Guard::Stale => {
                    return Err("runner guard used as native guard".into());
                }
            }
        }
    };
    let output = command
        .arg(format!("-DORACLE={}", std::env::var("OTTD_GRF_ORACLE")?))
        .arg(format!("-DRUN_DIR={}", native.display()))
        .arg(format!("-DMANIFEST={}", manifest.display()))
        .arg(format!("-DINPUT={input}"))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg("-DTICKS=1")
        .arg("-P")
        .arg(runner)
        .output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    verify_guard(directory, guard, &output, diagnostic, early)
}

fn verify_guard(
    directory: &Path,
    guard: Guard,
    output: &std::process::Output,
    diagnostic: &str,
    early: bool,
) -> Result {
    let native = directory.join("native");
    let observation = directory.join("safety.json");
    if matches!(guard, Guard::Unarmed) {
        if !output.status.success() || observation.exists() {
            return Err("unarmed observer executed or native failed".into());
        }
    } else {
        if output.status.success() {
            return Err("guard unexpectedly passed".into());
        }
        let message = if early {
            String::from_utf8_lossy(&output.stderr).into_owned()
        } else {
            std::fs::read_to_string(native.join("stderr.log"))?
        };
        if !message.contains(diagnostic) {
            return Err(format!("missing guard diagnostic {diagnostic}").into());
        }
    }
    if matches!(guard, Guard::Duplicate) && std::fs::read(&observation)? != b"sentinel\n" {
        return Err("duplicate output overwritten".into());
    }
    if early && !matches!(guard, Guard::Reuse) && native.exists() {
        return Err("early guard created native directory".into());
    }
    std::fs::write(
        directory.join("receipt.json"),
        serde_json::to_vec(
            &json!({"diagnostic":diagnostic,"early":early,"exit":output.status.code(),"accepted":false}),
        )?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original native safety observer"]
fn native_safety_host_guards() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_SAFETY_GUARDS")?);
    if !directory.is_absolute() {
        return Err("absolute guard artifact path required".into());
    }
    std::fs::create_dir(&directory)?;
    for (name, guard) in [
        ("replay", Guard::Replay),
        ("subset", Guard::Subset),
        ("reuse", Guard::Reuse),
        ("nonsave", Guard::NonSave),
        ("stale", Guard::Stale),
        ("duplicate", Guard::Duplicate),
        ("missing-input", Guard::MissingInput),
        ("missing-output", Guard::MissingOutput),
        ("unarmed", Guard::Unarmed),
    ] {
        run(&root, &directory.join(name), guard)?;
    }
    menu_control(&root, &directory.join("menu"))
}

fn menu_control(root: &Path, directory: &Path) -> Result {
    std::fs::create_dir(directory)?;
    let manifest = directory.join("manifest.json");
    std::fs::write(&manifest, b"[]\n")?;
    let observation = directory.join("safety.json");
    let script = directory.join("menu.cmake");
    std::fs::write(
        &script,
        r#"cmake_minimum_required(VERSION 3.20)
file(COPY_FILE "${CONFIG}" "${RUN_DIR}/openttd.cfg")
execute_process(COMMAND "${ORACLE}" -X -x -c "${RUN_DIR}/openttd.cfg" -vnull:ticks=1 -snull -mnull
    WORKING_DIRECTORY "${RUN_DIR}" OUTPUT_FILE "${RUN_DIR}/native-stdout.log" ERROR_FILE "${RUN_DIR}/native-stderr.log"
    RESULT_VARIABLE result TIMEOUT 15)
if(NOT result STREQUAL "0")
    message(FATAL_ERROR "Menu control failed: ${result}")
endif()
"#,
    )?;
    let output = Command::new("cmake")
        .env("OTTD_GRF_SAFETY_INPUT", &manifest)
        .env("OTTD_GRF_SAFETY_OUTPUT", &observation)
        .arg(format!("-DORACLE={}", std::env::var("OTTD_GRF_ORACLE")?))
        .arg(format!("-DRUN_DIR={}", directory.display()))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg("-P")
        .arg(script)
        .output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    if !output.status.success() || observation.exists() {
        return Err("menu executed safety observer or failed".into());
    }
    std::fs::write(
        directory.join("receipt.json"),
        serde_json::to_vec(
            &json!({"phase":"menu","exit":output.status.code(),"observation_exists":false}),
        )?,
    )?;
    Ok(())
}
