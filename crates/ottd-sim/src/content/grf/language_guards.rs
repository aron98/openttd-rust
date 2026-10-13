use super::{Result, cases, prepare};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[path = "language_guard_invocation.rs"]
mod invocation;
use invocation::{BodyKind, Guard, configure};

fn run(root: &Path, directory: &Path, input: (&cases::Case, Guard)) -> Result {
    let (case, guard) = input;
    let mut manifest = prepare(directory, case)?;
    let mut invocation = configure(root, directory, &mut manifest, &guard)?;
    let manifest_path = directory.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
    let args = vec![
        format!("-DORACLE={}", std::env::var("OTTD_GRF_LANGUAGE_ORACLE")?),
        format!("-DRUN_DIR={}", directory.join("native").display()),
        format!("-DINPUT={}", invocation.input),
        format!("-DMANIFEST={}", manifest_path.display()),
        format!("-DCONFIG={}", root.join("scripts/reference.cfg").display()),
        "-DTICKS=1".into(),
        "-P".into(),
        invocation.runner.display().to_string(),
    ];
    let environment = invocation
        .command
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
    let output = invocation.command.args(&args).output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    std::fs::write(
        directory.join("status.json"),
        serde_json::to_vec(&output.status.code())?,
    )?;
    if output.status.success() != invocation.success {
        return Err("unexpected guard exit".into());
    }
    let mut diagnostic = String::from_utf8_lossy(&output.stderr).into_owned();
    let native_stderr = directory.join("native/stderr.log");
    if native_stderr.exists() {
        diagnostic.push_str(&std::fs::read_to_string(native_stderr)?);
    }
    if !invocation.diagnostic.is_empty() && !diagnostic.contains(invocation.diagnostic) {
        return Err(format!("missing diagnostic {}", invocation.diagnostic).into());
    }
    match guard {
        Guard::DuplicateOutput => {
            if std::fs::read(&invocation.output)? != b"sentinel\n" {
                return Err("duplicate output overwritten".into());
            }
        }
        Guard::Replay
        | Guard::Subset
        | Guard::Reused
        | Guard::NonSave
        | Guard::Stale
        | Guard::MissingOutput
        | Guard::Selection
        | Guard::Body(_)
        | Guard::Unarmed => {
            if invocation.output.exists() || directory.join("native/language.json").exists() {
                return Err("guard produced language observation".into());
            }
        }
    }
    if matches!(guard, Guard::Selection | Guard::Body(_)) {
        rust_admission(directory, &manifest)?;
    }
    std::fs::write(
        directory.join("comparison.txt"),
        "PASS actual invocation status, diagnostic and observation boundary\n",
    )?;
    Ok(())
}

fn rust_admission(directory: &Path, manifest: &Value) -> Result {
    use crate::content::grf::{
        self,
        load::{RuntimeInputs, run_with_context},
        load_language_state::{LanguageInput, LanguageLimits},
    };
    let bytes = std::fs::read(directory.join("pack-0/input.lng"))?;
    let packs = [bytes.as_slice()];
    let selected = u8::try_from(
        manifest
            .pointer("/language/selected")
            .and_then(Value::as_u64)
            .ok_or("selection")?,
    )?;
    let error = run_with_context(
        &[],
        &[],
        grf::ControlOptions::default(),
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected,
                limits: LanguageLimits::default(),
            }),
        },
    )
    .err()
    .ok_or("Rust admitted native-refused language input")?;
    if !matches!(error, grf::ControlLoadError::InvalidNativeDomain { .. }) {
        return Err("wrong Rust language refusal type".into());
    }
    std::fs::write(directory.join("rust-refusal.txt"), format!("{error}\n"))?;
    Ok(())
}

#[test]
#[ignore = "requires original language observer and fresh absolute guard artifacts"]
fn original_language_guards() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_LANGUAGE_GUARDS")?);
    if !directory.is_absolute() {
        return Err("absolute guard artifacts required".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::copy(std::env::current_exe()?, directory.join("test-executable"))?;
    let oracle = PathBuf::from(std::env::var("OTTD_GRF_LANGUAGE_ORACLE")?);
    let bytes = std::fs::read(
        oracle
            .parent()
            .ok_or("oracle parent")?
            .join("lang/czech.lng"),
    )?;
    let case = cases::base("guard".into(), &bytes, &[])?;
    for (name, guard) in [
        ("replay", Guard::Replay),
        ("subset", Guard::Subset),
        ("reused", Guard::Reused),
        ("non-save", Guard::NonSave),
        ("stale", Guard::Stale),
        ("missing-output", Guard::MissingOutput),
        ("duplicate-output", Guard::DuplicateOutput),
        ("selection", Guard::Selection),
        ("body", Guard::Body(BodyKind::Truncated)),
        ("table", Guard::Body(BodyKind::Table)),
        ("last-extended", Guard::Body(BodyKind::LastExtended)),
        ("read-cap", Guard::Body(BodyKind::ReadCap)),
        ("unarmed", Guard::Unarmed),
    ] {
        run(&root, &directory.join(name), (&case, guard))
            .map_err(|error| format!("{name}: {error}"))?;
    }
    println!("passed 13 actual language host guards");
    Ok(())
}
