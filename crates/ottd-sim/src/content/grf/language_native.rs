use super::{
    language_pack::Pack,
    load::{RuntimeInputs, run_with_context},
    load_context_tests::{grf_control_cases as programs, native::project},
    load_language_state::{LanguageInput, LanguageLimits},
};
use serde_json::{Value, json};
use std::{path::Path, process::Command};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "language_body_cases.rs"]
mod bodies;
#[path = "language_cases.rs"]
mod cases;
#[path = "language_compare.rs"]
mod compare;
#[path = "language_guards.rs"]
mod guards;
#[path = "language_observations.rs"]
mod observations;
#[path = "language_ordered_cases.rs"]
mod ordered;
#[path = "language_text_cases.rs"]
mod text_cases;

fn field<'a>(value: &'a Value, pointer: &str) -> Result<&'a Value> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing {pointer}").into())
}

fn source_prefix(raw: &Value) -> Result<Vec<u32>> {
    field(raw, "/baseline_sources")?
        .as_array()
        .ok_or("baseline sources")?
        .iter()
        .map(|path| {
            Ok(
                super::scan_file(&std::fs::read(path.as_str().ok_or("baseline path")?)?)?
                    .metadata
                    .ok_or("baseline identity")?
                    .grfid,
            )
        })
        .collect()
}

fn mapping(pack: &Pack, gender: bool) -> Result<Vec<u8>> {
    let mut record = vec![0, 8, 1, 1, pack.language, if gender { 0x13 } else { 0x14 }];
    let slots: &[[u8; 16]] = if gender { &pack.genders } else { &pack.cases };
    for (index, name) in slots.iter().enumerate() {
        record.push(u8::try_from(index.checked_add(1).ok_or("index")?)?);
        record.extend(name.iter().copied().take_while(|byte| *byte != 0));
        record.push(0);
    }
    record.push(0);
    Ok(record)
}

fn invoke(root: &Path, directory: &Path, manifest: &Value) -> Result<Value> {
    let path = directory.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(manifest)?)?;
    let args = vec![
        format!("-DORACLE={}", std::env::var("OTTD_GRF_LANGUAGE_ORACLE")?),
        format!("-DRUN_DIR={}", directory.join("native").display()),
        format!("-DMANIFEST={}", path.display()),
        format!(
            "-DINPUT={}",
            root.join("fixtures/replay/clear-v362.sav").display()
        ),
        format!("-DCONFIG={}", root.join("scripts/reference.cfg").display()),
        "-P".into(),
        root.join("scripts/check-grf-language-reference.cmake")
            .display()
            .to_string(),
    ];
    std::fs::write(directory.join("argv.json"), serde_json::to_vec(&args)?)?;
    let output = Command::new("cmake").args(&args).output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    std::fs::write(
        directory.join("status.json"),
        serde_json::to_vec(&output.status.code())?,
    )?;
    if !output.status.success() {
        return Err("original language invocation failed".into());
    }
    Ok(serde_json::from_slice(&std::fs::read(
        directory.join("native/language.json"),
    )?)?)
}

fn prepare(directory: &Path, case: &cases::Case) -> Result<Value> {
    std::fs::create_dir(directory)?;
    let mut directories = Vec::new();
    for (index, bytes) in case.packs.iter().enumerate() {
        let path = directory.join(format!("pack-{index}"));
        std::fs::create_dir(&path)?;
        std::fs::write(path.join("input.lng"), bytes)?;
        directories.push(path);
    }
    let mut files = Vec::new();
    for source in &case.sources {
        let path = directory.join(&source.name);
        if let Some(bytes) = &source.bytes {
            std::fs::write(&path, bytes)?;
        }
        files.push(json!({"path":path,"grfid":source.id,"metadata_version":source.metadata_version,
            "parameters":source.parameters,"static":source.flags.is_static,"init_only":source.flags.init_only,"system":source.flags.system,"palette":1}));
    }
    Ok(json!({"networking":false,"files":files,
        "language":{"pack_directories":directories,"selected":case.selected,"queries":case.queries}}))
}

fn run(root: &Path, directory: &Path, case: &cases::Case) -> Result {
    let manifest = prepare(directory, case)?;
    let native = invoke(root, directory, &manifest)?;
    let control: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/control.json"))?)?;
    let prefix = source_prefix(&control)?;
    let packs = case.packs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let names = case
        .sources
        .iter()
        .map(|source| directory.join(&source.name).display().to_string())
        .collect::<Vec<_>>();
    let inputs = case
        .sources
        .iter()
        .zip(&names)
        .map(|(source, name)| {
            let mut input = source.input();
            input.name = name;
            input
        })
        .collect::<Vec<_>>();
    let (report, _, language) = run_with_context(
        &inputs,
        &prefix,
        super::ControlOptions::default(),
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected: case.selected,
                limits: LanguageLimits::default(),
            }),
        },
    )?;
    let language = language.ok_or("language report")?;
    let rust = project::rust(&report)?;
    std::fs::write(
        directory.join("rust-control.json"),
        serde_json::to_vec(&rust)?,
    )?;
    std::fs::write(
        directory.join("rust-language.json"),
        serde_json::to_vec(&language)?,
    )?;
    compare::checked(
        directory,
        "control",
        (&project::native(&control, &prefix)?, &rust),
    )?;
    observations::state(directory, &native, (&language, &prefix))?;
    let pack = language
        .catalog
        .iter()
        .find(|pack| pack.language == case.selected)
        .ok_or("selected pack")?;
    observations::text(directory, &native, &language, pack, &case.queries)?;
    std::fs::write(
        directory.join("comparison.txt"),
        "PASS original catalog, complete configured stage maps/control and mapped text bytes\n",
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original language observer and fresh absolute artifact directory"]
fn original_language_matrix() -> Result {
    for name in [
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_LANGUAGE_CASE",
        "OTTD_GRF_CONTROL_CASE",
    ] {
        if std::env::var_os(name).is_some() {
            return Err(format!("refused {name}").into());
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = std::path::PathBuf::from(std::env::var("OTTD_GRF_LANGUAGE_ARTIFACTS")?);
    if !directory.is_absolute() {
        return Err("absolute artifacts required".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::copy(std::env::current_exe()?, directory.join("test-executable"))?;
    let oracle = std::path::PathBuf::from(std::env::var("OTTD_GRF_LANGUAGE_ORACLE")?);
    let packs = oracle.parent().ok_or("oracle parent")?.join("lang");
    let cases = cases::all(&packs)?;
    std::fs::write(
        directory.join("cases.json"),
        serde_json::to_vec(&cases.iter().map(|case| &case.name).collect::<Vec<_>>())?,
    )?;
    for case in &cases {
        run(&root, &directory.join(&case.name), case)
            .map_err(|error| format!("{}: {error}", case.name))?;
    }
    println!(
        "compared {} original language catalog and stage witnesses",
        cases.len()
    );
    Ok(())
}
