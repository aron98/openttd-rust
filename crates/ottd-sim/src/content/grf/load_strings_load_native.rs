use super::{
    language_pack::Pack,
    load::{RuntimeInputs, run_with_context},
    load_context_tests::{grf_control_cases as programs, native::project},
    load_language_state::{LanguageInput, LanguageLimits, LanguageReport},
    load_strings_native::{checked, invoke},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[path = "load_strings_load_cases.rs"]
mod cases;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run_case(
    root: &Path,
    directory: &Path,
    oracle: &str,
    case: &programs::Case,
    pack: &[u8],
) -> Result<Value> {
    std::fs::create_dir(directory)?;
    let pack_directory = directory.join("pack");
    std::fs::create_dir(&pack_directory)?;
    std::fs::write(pack_directory.join("input.lng"), pack)?;
    let selected = Pack::header(pack)?.language;
    let mut files = Vec::new();
    let mut names = Vec::new();
    for source in &case.sources {
        let path = directory.join(&source.name);
        if let Some(bytes) = &source.bytes {
            std::fs::write(&path, bytes)?;
        }
        names.push(path.display().to_string());
        files.push(json!({"path":path,"grfid":source.id,"metadata_version":source.metadata_version,
            "parameters":source.parameters,"static":source.flags.is_static,"init_only":source.flags.init_only,
            "system":source.flags.system,"palette":1}));
    }
    let manifest = json!({"files":files,"networking":case.networking,"strings_load":true,
        "language":{"pack_directories":[pack_directory],"selected":selected,"queries":[]}});
    let native = invoke(root, directory, &manifest, oracle)?;
    let original_control: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/control.json"))?)?;
    let mut prefix = Vec::new();
    for path in original_control
        .get("baseline_sources")
        .and_then(Value::as_array)
        .ok_or("baseline sources")?
    {
        let bytes = std::fs::read(path.as_str().ok_or("baseline path")?)?;
        prefix.push(
            super::scan_file(&bytes)?
                .metadata
                .ok_or("baseline scan identity")?
                .grfid,
        );
    }
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
    let packs = [pack];
    let (report, _, language) = run_with_context(
        &inputs,
        &prefix,
        super::ControlOptions::default(),
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected,
                limits: LanguageLimits::default(),
            }),
        },
    )?;
    let language = language.ok_or("language state")?;
    let control = project::rust(&report)?;
    std::fs::write(
        directory.join("rust-control.json"),
        serde_json::to_vec(&control)?,
    )?;
    project::compare(&project::native(&original_control, &prefix)?, &control)?;
    let mut rust =
        json!(language.events.iter().map(|event| json!({
        "stage":event.stage,"file":event.file,"line":event.line,"offset":event.offset,
        "table":{"selected":selected,"entries":event.strings.iter().map(|entry| {
            json!([entry.key.grfid,entry.key.local_id,entry.default_id,entry.translations])
        }).collect::<Vec<_>>()},"translation_errors":event.translation_errors,
    })).collect::<Vec<_>>());
    std::fs::write(
        directory.join("rust-strings.json"),
        serde_json::to_vec(&rust)?,
    )?;
    let count = checked(
        native.get("events").ok_or("original string events")?,
        &mut rust,
        directory,
    )?;
    compare_final(directory, &native, &language)?;
    Ok(
        json!({"case":case.name,"events":language.events.len(),"strings":language.strings.len(),"controls":count}),
    )
}

fn compare_final(directory: &Path, native: &Value, language: &LanguageReport) -> Result {
    let final_table = json!({"selected":language.selected,"entries":language.strings.iter().map(|entry| {
        json!([entry.key.grfid,entry.key.local_id,entry.default_id,entry.translations])
    }).collect::<Vec<_>>()});
    project::compare(
        native.get("final_table").ok_or("final original table")?,
        &final_table,
    )?;
    project::compare(
        native
            .get("translation_errors")
            .ok_or("original translation errors")?,
        &json!(language.translation_errors),
    )?;
    std::fs::write(
        directory.join("rust-final.json"),
        serde_json::to_vec(&json!({
            "table":final_table,"translation_errors":language.translation_errors,
        }))?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original full scheduler string observer and fresh absolute artifact directory"]
fn original_string_load_matrix() -> Result {
    for name in [
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_STRINGS_CASE",
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_GRF_LANGUAGE_CASE",
    ] {
        if std::env::var_os(name).is_some() {
            return Err(format!("refused {name}").into());
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_STRINGS_ARTIFACTS")?);
    if !directory.is_absolute() {
        return Err("absolute artifacts required".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::copy(std::env::current_exe()?, directory.join("test-executable"))?;
    let oracle = std::env::var("OTTD_GRF_STRINGS_ORACLE")?;
    let languages = Path::new(&oracle)
        .parent()
        .ok_or("oracle parent")?
        .join("lang");
    let english = std::fs::read(languages.join("english.lng"))?;
    let mut summary = Vec::new();
    for case in cases::cases()? {
        summary.push(run_case(
            &root,
            &directory.join(&case.name),
            &oracle,
            &case,
            &english,
        )?);
    }
    let german = std::fs::read(languages.join("german.lng"))?;
    let mapped = cases::mapped(&Pack::header(&german)?)?;
    summary.push(run_case(
        &root,
        &directory.join(&mapped.name),
        &oracle,
        &mapped,
        &german,
    )?);
    let mut packs = std::fs::read_dir(languages)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    packs.retain(|path| path.extension().is_some_and(|extension| extension == "lng"));
    packs.sort();
    for path in packs {
        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("language filename")?;
        let mut case = cases::error_order()?;
        case.name = format!("localized-load-after-{stem}");
        let pack = std::fs::read(path)?;
        summary.push(run_case(
            &root,
            &directory.join(&case.name),
            &oracle,
            &case,
            &pack,
        )?);
    }
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(&summary)?,
    )?;
    println!(
        "compared {} original custom string load cases",
        summary.len()
    );
    Ok(())
}

#[test]
#[ignore = "requires original string observer and fresh absolute registry artifacts"]
fn original_string_registry_cases() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_STRINGS_ARTIFACTS")?);
    if !directory.is_absolute() {
        return Err("absolute artifacts required".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::copy(std::env::current_exe()?, directory.join("test-executable"))?;
    let oracle = std::env::var("OTTD_GRF_STRINGS_ORACLE")?;
    let pack = std::fs::read(
        Path::new(&oracle)
            .parent()
            .ok_or("oracle parent")?
            .join("lang/english.lng"),
    )?;
    let mut summary = Vec::new();
    for case in cases::registry_cases()? {
        summary.push(run_case(
            &root,
            &directory.join(&case.name),
            &oracle,
            &case,
            &pack,
        )?);
    }
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(&summary)?,
    )?;
    println!(
        "compared {} original string registry/no-op cases",
        summary.len()
    );
    Ok(())
}
