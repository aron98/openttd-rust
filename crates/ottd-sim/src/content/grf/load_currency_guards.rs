use super::load_currency_native::Result;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Copy)]
enum Guard {
    Menu,
    Replay,
    Subset,
    Reused,
    NonSave,
    Stale,
    Duplicate,
    Unarmed,
    Pending,
    StaleRead,
    Mixed,
    ReloadIndex,
    ReloadDuplicate,
}

fn manifest(directory: &Path, oracle: &str, guard: Guard) -> Result<PathBuf> {
    let bytes = std::fs::read(
        Path::new(oracle)
            .parent()
            .ok_or("oracle parent")?
            .join("lang/english.lng"),
    )?;
    let selected = super::language_pack::Pack::header(&bytes)?.language;
    let packs = directory.join("pack");
    std::fs::create_dir(&packs)?;
    std::fs::write(packs.join("input.lng"), bytes)?;
    let mut value = json!({"files":[],"networking":false,"currency_api":[],"language":{"pack_directories":[packs],"selected":selected,"queries":[]}});
    let fields = value.as_object_mut().ok_or("manifest object")?;
    match guard {
        Guard::Pending => {
            fields.insert("currency_api".into(),json!([{"operation":"queue","grfid":7,"first":0,"count":1,"reserve":false,"raw":[0,216]}]));
        }
        Guard::StaleRead => {
            fields.insert(
                "currency_api".into(),
                json!([{"operation":"read","id":131_072}]),
            );
        }
        Guard::Mixed => {
            fields.insert("currency_load".into(), json!(true));
        }
        Guard::ReloadIndex | Guard::ReloadDuplicate => {
            fields.remove("currency_api");
            fields.insert("currency_load".into(), json!(true));
            let mut files = Vec::new();
            for id in [7, 8] {
                let source = super::load_context_tests::grf_control_cases::source(id, &[], &[], 1)?;
                let path = directory.join(format!("input-{id}.grf"));
                std::fs::write(&path, source.bytes.ok_or("fixture bytes")?)?;
                files.push(json!({"path":path,"grfid":id,"metadata_version":1,"parameters":[],"static":false,"init_only":false,"system":false,"palette":1}));
            }
            fields.insert("files".into(), json!(files));
            fields.insert(
                "currency_reload".into(),
                if matches!(guard, Guard::ReloadIndex) {
                    json!([2])
                } else {
                    json!([0, 0])
                },
            );
        }
        Guard::Menu
        | Guard::Replay
        | Guard::Subset
        | Guard::Reused
        | Guard::NonSave
        | Guard::Stale
        | Guard::Duplicate
        | Guard::Unarmed => (),
    }
    let path = directory.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(&value)?)?;
    Ok(path)
}

fn stale(root: &Path, directory: &Path) -> Result<PathBuf> {
    let copied = directory.join("stale-source");
    for name in ["scripts", "reference"] {
        std::fs::create_dir_all(copied.join(name))?;
        for item in std::fs::read_dir(root.join(name))? {
            let item = item?;
            if item.file_type()?.is_file() {
                std::fs::copy(item.path(), copied.join(name).join(item.file_name()))?;
            }
        }
    }
    let header = copied.join("reference/grf_currency.hpp");
    let mut bytes = std::fs::read(&header)?;
    bytes.push(b'\n');
    std::fs::write(header, bytes)?;
    Ok(copied.join("scripts/check-grf-currency-reference.cmake"))
}

fn configure(
    root: &Path,
    directory: &Path,
    guard: Guard,
    command: &mut Command,
) -> Result<(PathBuf, String, &'static str)> {
    let mut runner = root.join("scripts/check-grf-currency-reference.cmake");
    let mut input = root
        .join("fixtures/replay/clear-v362.sav")
        .display()
        .to_string();
    let diagnostic = match guard {
        Guard::Menu => {
            runner = directory.join("menu.cmake");
            std::fs::write(
                &runner,
                r#"cmake_minimum_required(VERSION 3.20)
file(MAKE_DIRECTORY "${RUN_DIR}")
file(COPY_FILE "${CONFIG}" "${RUN_DIR}/openttd.cfg")
execute_process(COMMAND "${ORACLE}" -X -x -c "${RUN_DIR}/openttd.cfg" -vnull:ticks=1 -snull -mnull
    WORKING_DIRECTORY "${RUN_DIR}" OUTPUT_FILE "${RUN_DIR}/stdout.log" ERROR_FILE "${RUN_DIR}/stderr.log"
    RESULT_VARIABLE result TIMEOUT 15)
if(NOT result STREQUAL "0")
    message(FATAL_ERROR "Menu control failed: ${result}")
endif()
"#,
            )?;
            command.env("OTTD_GRF_CONTROL_MANIFEST", directory.join("manifest.json"));
            ""
        }
        Guard::Replay => {
            command.env("OTTD_REPLAY_PATH", directory.join("replay.json"));
            "refuses replay or case subset"
        }
        Guard::Subset => {
            command.env("OTTD_GRF_CURRENCY_CASE", "subset");
            "refuses replay or case subset"
        }
        Guard::Reused => {
            std::fs::create_dir(directory.join("native"))?;
            "run directory already exists"
        }
        Guard::NonSave => {
            input = "GENERATE".into();
            "requires an explicit saved-game load"
        }
        Guard::Stale => {
            runner = stale(root, directory)?;
            "Stale native replay binary/source"
        }
        Guard::Duplicate => {
            runner = root.join("scripts/check-grf-load-control-reference.cmake");
            command.env("OTTD_GRF_LANGUAGE_OUTPUT", directory.join("language.json"));
            std::fs::write(directory.join("currency.json"), b"sentinel\n")?;
            "currency output exists"
        }
        Guard::Unarmed => {
            runner = root.join("scripts/run-reference.cmake");
            input = "GENERATE".into();
            command.env_remove("OTTD_GRF_CONTROL_MANIFEST");
            ""
        }
        Guard::Pending => "currency API must finalize queued destinations",
        Guard::StaleRead => "stale custom currency string query",
        Guard::Mixed => "invalid or duplicate currency fixture",
        Guard::ReloadIndex | Guard::ReloadDuplicate => {
            "currency reload duplicate or invalid config"
        }
    };
    Ok((runner, input, diagnostic))
}

fn run(root: &Path, directory: &Path, oracle: &str, guard: Guard) -> Result {
    std::fs::create_dir(directory)?;
    let manifest = manifest(directory, oracle, guard)?;
    let mut command = Command::new("cmake");
    command.env("OTTD_GRF_CURRENCY_OUTPUT", directory.join("currency.json"));
    let (runner, input, diagnostic) = configure(root, directory, guard, &mut command)?;
    let args = vec![
        format!("-DORACLE={oracle}"),
        format!("-DRUN_DIR={}", directory.join("native").display()),
        format!("-DMANIFEST={}", manifest.display()),
        format!("-DINPUT={input}"),
        format!("-DCONFIG={}", root.join("scripts/reference.cfg").display()),
        "-DTICKS=1".into(),
        "-P".into(),
        runner.display().to_string(),
    ];
    let environment = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    std::fs::write(
        directory.join("argv.json"),
        serde_json::to_vec(&json!({"argv":args,"environment":environment}))?,
    )?;
    let output = command.args(&args).output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    std::fs::write(
        directory.join("status.json"),
        serde_json::to_vec(&output.status.code())?,
    )?;
    if output.status.success() != matches!(guard, Guard::Unarmed | Guard::Menu) {
        return Err("wrong native guard status".into());
    }
    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let native_log = directory.join("native/stderr.log");
    if native_log.exists() {
        stderr.push_str(&std::fs::read_to_string(native_log)?);
    }
    if !stderr.contains(diagnostic) {
        return Err(format!("missing diagnostic: {diagnostic}").into());
    }
    let observation = directory.join("currency.json");
    if matches!(guard, Guard::Duplicate) {
        if std::fs::read(observation)? != b"sentinel\n" {
            return Err("sentinel overwritten".into());
        }
    } else if observation.exists() {
        return Err("unexpected observation".into());
    }
    if directory.join("native/currency.json").exists() {
        return Err("unexpected runner observation".into());
    }
    std::fs::write(
        directory.join("comparison.txt"),
        "PASS actual status, diagnostic and output boundaries\n",
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original currency observer and fresh absolute guard artifacts"]
fn original_currency_guards() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_CURRENCY_GUARDS")?);
    if !directory.is_absolute() {
        return Err("absolute artifacts required".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::copy(std::env::current_exe()?, directory.join("test-executable"))?;
    let oracle = std::env::var("OTTD_GRF_CURRENCY_ORACLE")?;
    let guards = [
        ("menu", Guard::Menu),
        ("replay", Guard::Replay),
        ("subset", Guard::Subset),
        ("reused", Guard::Reused),
        ("non-save", Guard::NonSave),
        ("stale", Guard::Stale),
        ("duplicate", Guard::Duplicate),
        ("unarmed", Guard::Unarmed),
        ("pending", Guard::Pending),
        ("stale-read", Guard::StaleRead),
        ("mixed", Guard::Mixed),
        ("reload-index", Guard::ReloadIndex),
        ("reload-duplicate", Guard::ReloadDuplicate),
    ];
    for (name, guard) in guards {
        run(&root, &directory.join(name), &oracle, guard)
            .map_err(|error| format!("{name}: {error}"))?;
    }
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(
            &guards
                .iter()
                .map(|(name, _)| json!({"guard":name,"passed":true}))
                .collect::<Vec<Value>>(),
        )?,
    )?;
    println!("passed {} actual currency guards", guards.len());
    Ok(())
}
