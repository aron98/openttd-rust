//! Original scheduler witnesses for loading context and special targets.
use super::{Result, matrix};
use crate::content::grf::{
    ControlLoadReport, ControlOptions, FileControlState, LoadEvent, LoadStage, OverrideState,
    Palette,
    load::run_with_environment,
    load_context::{Environment, EnvironmentReport},
    scan_file,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
#[path = "load_context_guards.rs"]
mod guards;
#[path = "../../../tests/grf_control_cases/project.rs"]
pub mod project;

fn at<'a>(value: &'a Value, pointer: &str) -> Result<&'a Value> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing field {pointer}").into())
}

fn manifest(case: &matrix::Case, directory: &Path) -> Result<Value> {
    let mut files = Vec::new();
    for source in &case.program.sources {
        let path = directory.join(&source.name);
        if let Some(bytes) = &source.bytes {
            std::fs::write(&path, bytes)?;
        }
        files.push(json!({"path":path,"grfid":source.id,"metadata_version":source.metadata_version,
            "parameters":source.parameters,"static":source.flags.is_static,"init_only":source.flags.init_only,
            "system":source.flags.system,"palette":match case.palette { Palette::Dos => 0, Palette::Windows => 1 }}));
    }
    let e = case.environment;
    let s = e.settings;
    let p = s.patch;
    let climate = match s.climate {
        crate::content::Climate::Temperate => 0,
        crate::content::Climate::Arctic => 1,
        crate::content::Climate::Tropic => 2,
        crate::content::Climate::Toyland => 3,
    };
    Ok(
        json!({"networking":case.program.networking,"files":files,"context":{
            "calendar_date":e.saved.date.raw(),"calendar_fraction":e.saved.date_fract.0,
            "economy_date":e.saved.economy_date.0,"economy_fraction":e.saved.economy_date_fract.0,
            "tick":e.saved.tick_counter.0,"display":s.display_options,"networking":case.program.networking,
            "game_mode":s.game_mode,"timekeeping_units":match s.timekeeping_units {ottd_core::TimekeepingUnits::Calendar=>0,ottd_core::TimekeepingUnits::Wallclock=>1},
            "starting_year":s.starting_year,"climate":climate,"road_side":u8::from(s.right_hand_traffic),
            "disable_elrails":s.disable_elrails,"height_limit":s.height_limit,"snowline":s.snowline,
            "generation_seed":s.generation_seed,"freight_trains":s.freight_trains,"plane_speed":s.plane_speed,
            "map_width":s.map.width(),"map_height":s.map.height(),"never_expire_airports":p.never_expire_airports,
            "max_bridge_length":p.max_bridge_length,"never_expire_vehicles":p.never_expire_vehicles,
            "station_noise_level":p.station_noise_level,"gradual_loading":p.gradual_loading,"train_signal_side":p.train_signal_side,
            "build_on_slopes":p.build_on_slopes,"wagon_speed_limits":p.wagon_speed_limits,"allow_town_roads":p.allow_town_roads,
            "generating_world":p.generating_world,"improved_load":p.improved_load,"dynamic_engines":p.dynamic_engines,"inflation":p.inflation
        }}),
    )
}

fn prefix(raw: &Value) -> Result<Vec<u32>> {
    let paths = at(raw, "/baseline_sources")?
        .as_array()
        .ok_or("baseline paths")?;
    let mut ids = Vec::new();
    for path in paths {
        let bytes = std::fs::read(path.as_str().ok_or("baseline path")?)?;
        ids.push(
            scan_file(&bytes)?
                .metadata
                .ok_or("baseline metadata")?
                .grfid,
        );
    }
    let native = at(raw, "/before/configs")?
        .as_array()
        .ok_or("baseline configs")?;
    if ids.len() != native.len() {
        return Err("baseline length mismatch".into());
    }
    for (id, config) in ids.iter().zip(native) {
        if at(config, "/grfid")? != &json!(id) {
            return Err("baseline identity mismatch".into());
        }
    }
    Ok(ids)
}

fn compare_context(raw: &Value, report: &EnvironmentReport, case: &matrix::Case) -> Result {
    let final_environment = &report.0;
    let files = at(raw, "/files")?
        .as_array()
        .ok_or("native files")?
        .get(2..)
        .ok_or("native prefix files")?;
    if files != projected_files(report) {
        return Err("final filename-associated context differs".into());
    }
    if at(raw, "/before")? != at(raw, "/restored")? {
        return Err("fixture changed original context".into());
    }
    for field in [
        "calendar_date",
        "calendar_year",
        "calendar_fraction",
        "economy_date",
        "economy_year",
        "economy_fraction",
        "tick",
        "display",
        "random",
        "interactive_random",
    ] {
        if at(raw, &format!("/prepared/{field}"))? != at(raw, &format!("/after_native/{field}"))? {
            return Err(format!("loader changed {field}").into());
        }
    }
    if at(raw, "/globals")?
        != &json!({"rail_costs":final_environment.rail_costs,"misc":final_environment.misc})
    {
        return Err("final global state mismatch".into());
    }
    let normalized = Environment::new(
        case.environment.saved,
        case.environment.settings,
        case.program.networking,
    )?;
    let stages = at(raw, "/stages")?.as_array().ok_or("stages")?;
    if stages.len() != 4 {
        return Err("stage count".into());
    }
    for (stage, expected_stage) in stages.iter().zip(2..=5) {
        if at(stage, "/stage")? != &json!(expected_stage) {
            return Err("stage order".into());
        }
        for (field, expected) in [
            ("calendar_date", json!(normalized.current.date.raw())),
            ("calendar_fraction", json!(normalized.current.date_fract.0)),
            ("economy_date", json!(normalized.current.economy_date.0)),
            (
                "economy_fraction",
                json!(normalized.current.economy_date_fract.0),
            ),
            ("tick", json!(normalized.current.tick_counter.0)),
            ("display", json!(normalized.current_display)),
        ] {
            if at(stage, &format!("/context/{field}"))? != &expected {
                return Err(format!("stage context {field}").into());
            }
        }
    }
    if final_environment.current != case.environment.saved {
        return Err("Rust clock restoration".into());
    }
    if final_environment.current_display != case.environment.settings.display_options {
        return Err("Rust display restoration".into());
    }
    Ok(())
}

fn projected_files(report: &EnvironmentReport) -> Vec<Value> {
    report
        .1
        .iter()
        .map(|file| {
            file.map_or(
                Value::Null,
                |file| json!({"pitch":file.pitch,"width":file.width}),
            )
        })
        .collect()
}

fn run(root: &Path, directory: &Path, case: &matrix::Case) -> Result {
    std::fs::create_dir(directory)?;
    let input_path = input_save(root, directory, case)?;
    let manifest_path = directory.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest(case, directory)?)?,
    )?;
    let context_path = directory.join("context.json");
    let output = Command::new("cmake")
        .env("OTTD_GRF_CONTEXT_OUTPUT", &context_path)
        .arg(format!(
            "-DORACLE={}",
            std::env::var("OTTD_GRF_CONTEXT_ORACLE")?
        ))
        .arg(format!("-DRUN_DIR={}", directory.join("native").display()))
        .arg(format!("-DMANIFEST={}", manifest_path.display()))
        .arg(format!("-DINPUT={}", input_path.display()))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg("-P")
        .arg(root.join("scripts/check-grf-load-control-reference.cmake"))
        .output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err("native execution failed".into());
    }
    let raw: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/control.json"))?)?;
    let prefix = prefix(&raw)?;
    let inputs = case
        .program
        .sources
        .iter()
        .map(|source| {
            let mut input = source.input();
            input.palette = case.palette;
            input
        })
        .collect::<Vec<_>>();
    let (report, environment) = run_with_environment(
        &inputs,
        &prefix,
        ControlOptions {
            networking: case.program.networking,
            ..Default::default()
        },
        Some(case.environment),
    )?;
    let rust = project::rust(&report)?;
    let native = project::native(&raw, &prefix)?;
    std::fs::write(directory.join("rust.json"), serde_json::to_vec(&rust)?)?;
    std::fs::write(
        directory.join("native-normalized.json"),
        serde_json::to_vec(&native)?,
    )?;
    project::compare(&native, &rust)?;
    let pointer = project::field_pointer(&rust, "parameters", "").ok_or("parameter witness")?;
    let mut altered_trace = rust;
    let parameter = altered_trace
        .pointer_mut(&pointer)
        .ok_or("parameter pointer")?;
    *parameter = project::changed(parameter);
    std::fs::write(
        directory.join("altered-trace.json"),
        serde_json::to_vec(&altered_trace)?,
    )?;
    if project::compare(&native, &altered_trace).is_ok() {
        return Err("altered trace accepted".into());
    }
    let context: Value = serde_json::from_slice(&std::fs::read(context_path)?)?;
    let environment = environment.ok_or("Rust environment")?;
    std::fs::write(
        directory.join("rust-context.json"),
        serde_json::to_vec(
            &json!({"files":projected_files(&environment),"globals":{"rail_costs":environment.0.rail_costs,"misc":environment.0.misc},"clock":{"calendar_date":environment.0.current.date.raw(),"calendar_fraction":environment.0.current.date_fract.0,"economy_date":environment.0.current.economy_date.0,"economy_fraction":environment.0.current.economy_date_fract.0,"tick":environment.0.current.tick_counter.0,"display":environment.0.current_display}}),
        )?,
    )?;
    compare_context(&context, &environment, case)?;
    context_controls(directory, &context, &environment, case)?;
    Ok(())
}

fn context_controls(
    directory: &Path,
    context: &Value,
    environment: &EnvironmentReport,
    case: &matrix::Case,
) -> Result {
    let mut pointers = [
        "/globals/misc",
        "/globals/rail_costs/0",
        "/files/2/pitch",
        "/files/2/width",
        "/before/tick",
        "/after_native/calendar_date",
    ]
    .map(str::to_owned)
    .to_vec();
    for stage in 0..4 {
        for field in [
            "calendar_date",
            "calendar_fraction",
            "economy_date",
            "economy_fraction",
            "tick",
            "display",
        ] {
            pointers.push(format!("/stages/{stage}/context/{field}"));
        }
    }
    let mut controls = Vec::new();
    for pointer in pointers {
        let mut altered = context.clone();
        let field = altered
            .pointer_mut(&pointer)
            .ok_or("context control field")?;
        let before = field.clone();
        *field = project::changed(field);
        let after = field.clone();
        if compare_context(&altered, environment, case).is_ok() {
            return Err(format!("altered context accepted at {pointer}").into());
        }
        controls.push(
            json!({"pointer":pointer,"before":before,"after":after,"comparator_rejected":true}),
        );
    }
    std::fs::write(
        directory.join("context-controls.json"),
        serde_json::to_vec(&controls)?,
    )?;
    std::fs::write(
        directory.join("comparison.txt"),
        format!(
            "PASS exact original control trace, clocks, globals and filename-associated context; {} context mutations and one trace mutation rejected\n",
            controls.len()
        ),
    )?;
    Ok(())
}

fn input_save(root: &Path, directory: &Path, case: &matrix::Case) -> Result<PathBuf> {
    let map = case.environment.settings.map;
    if map.width() == 64 && map.height() == 64 {
        return Ok(root.join("fixtures/replay/clear-v362.sav"));
    }
    let config = std::fs::read_to_string(root.join("scripts/reference.cfg"))?
        .replace("map_x = 6", &format!("map_x = {}", map.width().ilog2()))
        .replace("map_y = 6", &format!("map_y = {}", map.height().ilog2()));
    let config_path = directory.join("generation.cfg");
    std::fs::write(&config_path, config)?;
    let generation = directory.join("generation");
    let output = Command::new("cmake")
        .env_remove("OTTD_GRF_CONTROL_MANIFEST")
        .env_remove("OTTD_GRF_CONTROL_OUTPUT")
        .env_remove("OTTD_GRF_CONTEXT_OUTPUT")
        .arg(format!(
            "-DORACLE={}",
            std::env::var("OTTD_GRF_CONTEXT_ORACLE")?
        ))
        .arg(format!("-DRUN_DIR={}", generation.display()))
        .arg(format!("-DCONFIG={}", config_path.display()))
        .args(["-DINPUT=GENERATE", "-DTICKS=1", "-P"])
        .arg(root.join("scripts/run-reference.cmake"))
        .output()?;
    std::fs::write(directory.join("generation-stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("generation-stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err("native map generation failed".into());
    }
    Ok(generation.join("save/autosave/exit.sav"))
}

#[test]
#[ignore = "requires the original context observer and fresh artifact directory"]
fn original_loader_context_matrix() -> Result {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_CONTEXT_ARTIFACTS")?);
    if !directory.is_absolute() {
        return Err("artifact directory must be absolute".into());
    }
    if std::env::var_os("OTTD_REPLAY_PATH").is_some() {
        return Err("context matrix requires dedicated invocation".into());
    }
    let freshness = Command::new("cmake")
        .arg(format!(
            "-DORACLE={}",
            std::env::var("OTTD_GRF_CONTEXT_ORACLE")?
        ))
        .arg("-P")
        .arg(root.join("scripts/check-replay-build.cmake"))
        .output()?;
    if !freshness.status.success() {
        return Err(format!(
            "stale context oracle: {}",
            String::from_utf8_lossy(&freshness.stderr)
        )
        .into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::write(directory.join("freshness-stdout.log"), &freshness.stdout)?;
    std::fs::write(directory.join("freshness-stderr.log"), &freshness.stderr)?;
    std::fs::copy(
        std::env::current_exe()?,
        directory.join("context-test-binary"),
    )?;
    let cases = matrix::all()?;
    for case in &cases {
        run(&root, &directory.join(&case.program.name), case)
            .map_err(|error| format!("{}: {error}", case.program.name))?;
    }
    println!("compared {} original context invocations", cases.len());
    Ok(())
}
