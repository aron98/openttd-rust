use serde_json::json;
use std::{path::Path, process::Command};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
enum Guard {
    Replay,
    Subset,
    Reused,
    NonSave,
    Stale,
    Duplicate,
    Unarmed,
}

fn stale(root: &Path, directory: &Path) -> Result<std::path::PathBuf> {
    let source = directory.join("stale-source");
    for name in ["scripts", "reference"] {
        std::fs::create_dir_all(source.join(name))?;
        for item in std::fs::read_dir(root.join(name))? {
            let item = item?;
            if item.file_type()?.is_file() {
                std::fs::copy(item.path(), source.join(name).join(item.file_name()))?;
            }
        }
    }
    let header = source.join("reference/grf_strings.hpp");
    let mut bytes = std::fs::read(&header)?;
    bytes.push(b'\n');
    std::fs::write(header, bytes)?;
    Ok(source.join("scripts/check-grf-strings-reference.cmake"))
}

fn prepare(directory: &Path, oracle: &str) -> Result<std::path::PathBuf> {
    let pack = Path::new(oracle)
        .parent()
        .ok_or("oracle parent")?
        .join("lang/english.lng");
    let bytes = std::fs::read(pack)?;
    let selected = super::language_pack::Pack::header(&bytes)?.language;
    let packs = directory.join("pack");
    std::fs::create_dir(&packs)?;
    std::fs::write(packs.join("input.lng"), bytes)?;
    let manifest = directory.join("manifest.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&json!({
            "files":[],"networking":false,"strings_api":[{"operation":"reset"}],
            "language":{"pack_directories":[packs],"selected":selected,"queries":[]}
        }))?,
    )?;
    Ok(manifest)
}

fn run(root: &Path, directory: &Path, guard: Guard) -> Result {
    std::fs::create_dir(directory)?;
    let oracle = std::env::var("OTTD_GRF_STRINGS_ORACLE")?;
    let manifest = prepare(directory, &oracle)?;
    let observation = directory.join("strings.json");
    let mut command = Command::new("cmake");
    command.env("OTTD_GRF_STRINGS_OUTPUT", &observation);
    let mut runner = root.join("scripts/check-grf-strings-reference.cmake");
    let mut input = root
        .join("fixtures/replay/clear-v362.sav")
        .display()
        .to_string();
    let diagnostic = match guard {
        Guard::Replay => {
            command.env("OTTD_REPLAY_PATH", directory.join("replay.json"));
            "refuses replay or case subset"
        }
        Guard::Subset => {
            command.env("OTTD_GRF_STRINGS_CASE", "subset");
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
            std::fs::write(&observation, b"sentinel\n")?;
            "output exists"
        }
        Guard::Unarmed => {
            runner = root.join("scripts/run-reference.cmake");
            input = "GENERATE".into();
            command.env_remove("OTTD_GRF_CONTROL_MANIFEST");
            ""
        }
    };
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
    if output.status.success() != matches!(guard, Guard::Unarmed) {
        return Err("wrong guard process status".into());
    }
    let mut errors = String::from_utf8_lossy(&output.stderr).into_owned();
    let native_log = directory.join("native/stderr.log");
    if native_log.exists() {
        errors.push_str(&std::fs::read_to_string(native_log)?);
    }
    if !errors.contains(diagnostic) {
        return Err(format!("missing guard diagnostic {diagnostic}").into());
    }
    check_output(directory, guard)?;
    std::fs::write(
        directory.join("comparison.txt"),
        "PASS actual guard status, diagnostic, and output boundary\n",
    )?;
    Ok(())
}

fn check_output(directory: &Path, guard: Guard) -> Result {
    let observation = directory.join("strings.json");
    if matches!(guard, Guard::Duplicate) {
        if std::fs::read(observation)? != b"sentinel\n" {
            return Err("overwritten sentinel".into());
        }
    } else if observation.exists() {
        return Err("unwanted string observation".into());
    }
    if directory.join("native/strings.json").exists() {
        return Err("unwanted runner observation".into());
    }
    Ok(())
}

#[test]
#[ignore = "requires original string observer and fresh absolute guard directory"]
fn original_string_guards() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = std::path::PathBuf::from(std::env::var("OTTD_GRF_STRINGS_GUARDS")?);
    if !directory.is_absolute() {
        return Err("absolute guard artifacts required".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::copy(std::env::current_exe()?, directory.join("test-executable"))?;
    for (name, guard) in [
        ("replay", Guard::Replay),
        ("subset", Guard::Subset),
        ("reused", Guard::Reused),
        ("non-save", Guard::NonSave),
        ("stale", Guard::Stale),
        ("duplicate", Guard::Duplicate),
        ("unarmed", Guard::Unarmed),
    ] {
        run(&root, &directory.join(name), guard).map_err(|error| format!("{name}: {error}"))?;
    }
    println!("passed 7 actual custom string host guards");
    Ok(())
}
