use super::super::{
    ControlOptions, LoadLocation, LoadStage,
    load::{ActionError, ActionResult},
    load_budget::Budget,
    load_cargo::{CargoState, Change, FileIdentity},
    load_cargo_translation::{Request, Table},
    load_registry::Registry,
    load_specs::{RoadRequest, Specs},
    records::Reader,
};
use super::{Result, engine_units, programs};
use serde::Deserialize;
use std::path::Path;
#[path = "load_cargo_native_driver.rs"]
mod driver;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Case {
    name: String,
    manifest: Value,
}

fn location(stage: LoadStage, file: usize) -> LoadLocation {
    LoadLocation {
        stage,
        file,
        line: 1,
        offset: 0,
    }
}

fn projection(cargo: &CargoState, registry: &Registry<'_>, specs: &Specs) -> Result<Value> {
    let mut engines = engine_units::projection(specs)?;
    let owners = engines
        .get_mut("owners")
        .and_then(Value::as_array_mut)
        .ok_or("engine owners")?;
    for owner in owners {
        let identity = owner
            .get("grfid")
            .and_then(Value::as_u64)
            .map(u32::try_from)
            .transpose()?;
        if let Some(grfid) = identity {
            let index = registry
                .files
                .iter()
                .position(|file| file.grfid == grfid)
                .ok_or("owner file")?;
            owner
                .as_object_mut()
                .ok_or("owner")?
                .insert("grfid".into(), json!(FileIdentity { index, grfid }));
        }
    }
    let temporary = engines
        .get_mut("temporary")
        .and_then(Value::as_array_mut)
        .ok_or("temporary")?;
    for (value, raw) in temporary.iter_mut().zip(&specs.temporary) {
        value
            .as_object_mut()
            .ok_or("temporary")?
            .insert("defaultcargo_grfid".into(), json!(raw.defaultcargo_file));
    }
    let owners = cargo
        .owners
        .iter()
        .enumerate()
        .map(|(id, owner)| {
            let mut value = serde_json::to_value(owner)?;
            let fields = value.as_object_mut().ok_or("cargo owner")?;
            fields.insert("id".into(), json!(id));
            fields.insert("group".into(), Value::Null);
            fields.insert("valid".into(), json!(owner.spec.bitnum != u8::MAX));
            fields.insert(
                "default".into(),
                json!(cargo.default_labels.contains(&owner.spec.label)),
            );
            Ok(value)
        })
        .collect::<Result<Vec<_>>>()?;
    let files = registry.files.iter().enumerate().map(|(index,file)| {
        let table = file.cargo.as_ref().ok_or("table")?;
        Ok(json!({"index":index,"grfid":file.grfid,"version":file.version,"parameters":file.parameters,"features":file.features,"cargo_list":table.cargo_list,"fallback":table.fallback,"cargo_map":table.cargo_map,"selected_table":table.selected(file.version,cargo)}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"cargo":owners,"cargo_mask":cargo.cargo_mask,"standard_cargo_mask":cargo.standard_cargo_mask,"label_map":cargo.label_map.iter().map(|(&label,&id)|json!([label,id])).collect::<Vec<_>>(),"climate_dependent":cargo.climate_dependent,"climate_independent":cargo.climate_independent,"files":files,"engines":engines}),
    )
}

fn compare(actual: &Value, event: &Value, case: &str, phase: &str) -> Result {
    let mut expected = event.get("state").ok_or("native state")?.clone();
    expected
        .get_mut("engines")
        .and_then(Value::as_object_mut)
        .ok_or("native engines")?
        .remove("context");
    assert_eq!(actual, &expected, "{case}/{phase}");
    Ok(())
}

fn number(value: &Value, name: &str) -> Result<u32> {
    Ok(u32::try_from(
        value.get(name).and_then(Value::as_u64).ok_or("number")?,
    )?)
}

fn result_detail(result: ActionResult<Change>, reader: &Reader<'_>) -> Result<Value> {
    let mut value = match result {
        Ok(change) => {
            json!({"result":match change { Change::Success=>0, Change::Unhandled=>2, Change::Unknown=>3, Change::InvalidId=>4 }})
        }
        Err(ActionError::Bounds) => json!({"read_bounds":true}),
        Err(ActionError::Host(error)) => return Err(error.into()),
    };
    value
        .as_object_mut()
        .ok_or("detail")?
        .insert("remaining".into(), json!(reader.remaining()));
    Ok(value)
}

fn run_case(case: &Case, directory: &Path) -> Result<Vec<Value>> {
    let input = directory.join("native/cargo.json");
    assert!(input.metadata()?.len() <= 33 * 1024 * 1024);
    let bytes = std::fs::read(input)?;
    let native: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(native.get("input"), case.manifest.get("cargo_identity"));
    let events = native
        .get("events")
        .and_then(Value::as_array)
        .ok_or("events")?;
    let mut budget = Budget::new(ControlOptions::default());
    let predecessor = if case.name == "loader-baseline" {
        "before-reset"
    } else {
        "after-finalize"
    };
    let inherited = events
        .iter()
        .find(|event| event.get("phase").and_then(Value::as_str) == Some(predecessor))
        .and_then(|event| event.get("state"))
        .and_then(|state| state.get("standard_cargo_mask"))
        .and_then(Value::as_u64)
        .ok_or("inherited standard mask")?;
    let cargo = CargoState::new(
        crate::content::Climate::Temperate,
        inherited,
        &mut budget,
        location(LoadStage::Reserve, 0),
    )?;
    let specs = Specs::new(true, location(LoadStage::Reserve, 0))?;
    let mut registry = Registry::new(&[]);
    if case.name == "loader-baseline" {
        let event = events
            .iter()
            .find(|event| event.get("phase").and_then(Value::as_str) == Some("after-reset"))
            .ok_or("reset")?;
        let state = projection(&cargo, &registry, &specs)?;
        compare(&state, event, &case.name, "after-reset")?;
        return Ok(vec![
            json!({"phase":"after-reset","detail":null,"state":state}),
        ]);
    }
    let input = native.get("input").ok_or("input")?;
    let files = input
        .get("api_files")
        .and_then(Value::as_array)
        .ok_or("api files")?;
    let sources = files
        .iter()
        .map(|file| programs::source(number(file, "grfid")?, &[], &[], 1))
        .collect::<Result<Vec<_>>>()?;
    let inputs = sources
        .iter()
        .map(programs::Source::input)
        .collect::<Vec<_>>();
    registry = Registry::new(&inputs);
    for (index, input) in inputs.iter().enumerate() {
        registry.initialize(*input);
        let file = registry.file_mut(index).ok_or("file")?;
        file.version = u8::try_from(number(files.get(index).ok_or("api file")?, "version")?)?;
        file.cargo = Some(Table::default());
    }
    let commands = input
        .get("commands")
        .and_then(Value::as_array)
        .ok_or("commands")?;
    let observed = events
        .iter()
        .filter(|event| {
            matches!(
                event.get("phase").and_then(Value::as_str),
                Some(
                    "api-command"
                        | "property-enter"
                        | "property-return"
                        | "road-owner-resolved"
                        | "api-finish"
                )
            )
        })
        .collect();
    let mut harness = driver::Harness {
        cargo,
        registry,
        specs,
        budget,
        events: observed,
        compared: 0,
        name: &case.name,
        rows: Vec::new(),
    };
    for command in commands {
        harness.command(command)?;
    }
    harness.finish()?;
    assert_eq!(harness.rows.len(), harness.compared);
    Ok(harness.rows)
}

#[test]
#[ignore = "requires fresh source-bound original cargo corpus"]
fn cargo_native_complete_raw_states() -> Result {
    let directory = std::env::var("OTTD_CARGO_IDENTITY_CASES")?;
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    let cases: Vec<Case> = serde_json::from_str(include_str!("load_cargo_native_cases.json"))?;
    assert_eq!(cases.len(), 9);
    for case in &cases {
        let rows = run_case(case, &directory.join(&case.name))?;
        let destination = directory.join(&case.name).join("rust.json");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        serde_json::to_writer(file, &rows)?;
        println!("{}: {} full raw snapshots compared", case.name, rows.len());
    }
    Ok(())
}
