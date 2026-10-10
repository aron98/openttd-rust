use super::super::{
    LoadLocation, LoadStage,
    load::ActionError,
    load_budget::Budget,
    load_context_tests::native::project,
    load_engine_mapping::Kind,
    load_specs::{RoadRequest, RoadResult, Specs},
    records::Reader,
};
use super::{ControlOptions, Result, engine_units::projection, programs, run};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    mode: Mode,
    dynamic_engines: bool,
    commands: Vec<Operation>,
    files: Vec<File>,
}

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Api,
    Load,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    grfid: u32,
    bytes: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Operation {
    ResetDefault,
    ResetRetained,
    Snapshot,
    Acquire {
        grfid: u32,
        local_id: u16,
        #[serde(rename = "static")]
        static_access: bool,
    },
    Property {
        grfid: u32,
        first: u16,
        count: u8,
        property: u8,
        raw: Vec<u8>,
        stage: Stage,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Stage {
    Reserve,
    Activation,
}

fn location() -> LoadLocation {
    LoadLocation {
        stage: LoadStage::Activation,
        file: 0,
        line: 1,
        offset: 0,
    }
}

fn api(case: &Case) -> Result<Value> {
    let mut state = Specs::new(case.dynamic_engines, location())?;
    let mut budget = Budget::new(ControlOptions::default());
    let mut rows = Vec::new();
    for operation in &case.commands {
        let mut detail = json!({"command":operation});
        match operation {
            Operation::ResetDefault => state = Specs::new(case.dynamic_engines, location())?,
            Operation::ResetRetained => state.reset(location())?,
            Operation::Snapshot => (),
            Operation::Acquire {
                grfid,
                local_id,
                static_access,
            } => {
                let engine =
                    state.acquire(*grfid, Kind::Road, *local_id, *static_access, location())?;
                detail
                    .as_object_mut()
                    .ok_or("detail")?
                    .insert("engine".into(), json!(engine));
            }
            Operation::Property {
                grfid,
                first,
                count,
                property,
                raw,
                stage,
            } => {
                let mut reader = Reader { bytes: raw, pos: 0 };
                let fields = detail.as_object_mut().ok_or("detail")?;
                let request = RoadRequest {
                    grfid: *grfid,
                    first: u32::from(*first),
                    count: u32::from(*count),
                    property: *property,
                };
                let location = LoadLocation {
                    stage: match stage {
                        Stage::Reserve => LoadStage::Reserve,
                        Stage::Activation => LoadStage::Activation,
                    },
                    ..location()
                };
                match super::super::load_specs::dispatch_road(
                    Some(&mut state),
                    request,
                    &mut reader,
                    &mut budget,
                    location,
                ) {
                    Ok(result) => {
                        fields.insert(
                            "result".into(),
                            json!(match result {
                                RoadResult::Success => 0,
                                RoadResult::Unknown => 3,
                                RoadResult::Unhandled => 2,
                            }),
                        );
                    }
                    Err(ActionError::Bounds) => {
                        fields.insert("read_bounds".into(), json!(true));
                    }
                    Err(ActionError::Host(error)) => return Err(error.into()),
                }
                fields.insert("remaining".into(), json!(reader.remaining()));
            }
        }
        budget.trace(state.snapshot_bytes(), location())?;
        rows.push(json!({"detail":detail,"state":projection(&state)?}));
    }
    Ok(json!({"mode":"api","results":rows,"files":[]}))
}

fn load(case: &Case) -> Result<Value> {
    let sources = case
        .files
        .iter()
        .enumerate()
        .map(|(index, file)| programs::Source {
            name: format!("{index}.grf"),
            bytes: Some(file.bytes.clone()),
            id: file.grfid,
            parameters: Vec::new(),
            flags: super::super::LoadFlags::default(),
            metadata_version: 0,
        })
        .collect::<Vec<_>>();
    let (control, language) = run(&sources, None, ControlOptions::default())?;
    let state = match language.specs {
        Some(state) => state,
        None if case.files.is_empty() => Specs::new(case.dynamic_engines, location())?,
        None => return Err("missing executed engine state".into()),
    };
    let control = project::rust(&control)?;
    Ok(
        json!({"mode":"load","results":[{"detail":null,"state":projection(&state)?}],"files":control.get("files").ok_or("files")?}),
    )
}

fn write_case(case: &Case, root: &Path) -> Result {
    let directory = root.join(&case.name);
    std::fs::create_dir(&directory)?;
    let mut files = Vec::new();
    for (index, file) in case.files.iter().enumerate() {
        let path = directory.join(format!("{index}.grf"));
        std::fs::write(&path, &file.bytes)?;
        files.push(json!({"path":path,"grfid":file.grfid,"metadata_version":0,"parameters":[],"static":false,"init_only":false,"system":false}));
    }
    let mut spec = json!({"mode":case.mode,"dynamic_engines":case.dynamic_engines});
    let rust = match case.mode {
        Mode::Api => {
            spec.as_object_mut()
                .ok_or("spec")?
                .insert("commands".into(), json!(case.commands));
            api(case)?
        }
        Mode::Load => load(case)?,
    };
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec(&json!({"networking":false,"files":files,"engine_specs":spec}))?,
    )?;
    std::fs::write(directory.join("rust.json"), serde_json::to_vec(&rust)?)?;
    Ok(())
}

#[test]
#[ignore = "explicit bounded engine/spec corpus producer"]
fn original_engine_spec_corpus() -> Result {
    let destination =
        std::env::var_os("OTTD_ENGINE_SPECS_CASES").ok_or("missing fresh engine cases path")?;
    let destination = Path::new(&destination);
    if !destination.is_absolute() || destination.exists() {
        return Err("engine cases require absent absolute path".into());
    }
    let corpus: Corpus = serde_json::from_str(include_str!("load_engine_ci_cases.json"))?;
    let names = corpus
        .cases
        .iter()
        .map(|case| case.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "loader-baseline",
            "api-dynamic-on",
            "api-dynamic-off",
            "loader-scalars",
            "loader-truncated",
            "loader-unknown",
            "loader-unknown-followup",
            "loader-recognized-prefix"
        ]
    );
    std::fs::create_dir(destination)?;
    for case in &corpus.cases {
        write_case(case, destination)?;
    }
    std::fs::write(
        destination.join("summary.json"),
        serde_json::to_vec(&names)?,
    )?;
    Ok(())
}
