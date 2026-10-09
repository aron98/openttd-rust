//! Original whole-loader execution compared with the bounded control profile.
pub mod grf_control_cases;
use grf_control_cases::{Case, Result};
#[path = "grf_control_cases/project.rs"]
pub mod project;
use ottd_sim::content::grf::{
    ControlOptions, run_control_load, run_control_load_with_prefix, scan_file,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn generated_control_programs_are_admitted() -> Result {
    let cases = grf_control_cases::all()?;
    for case in &cases {
        let inputs = case
            .sources
            .iter()
            .map(grf_control_cases::Source::input)
            .collect::<Vec<_>>();
        let report = run_control_load(
            &inputs,
            ControlOptions {
                networking: case.networking,
                ..ControlOptions::default()
            },
        )
        .map_err(|error| format!("{}: {error}", case.name))?;
        project::rust(&report)?;
    }
    println!("admitted {} generated control programs", cases.len());
    Ok(())
}
fn manifest(case: &Case, directory: &Path) -> Result<Value> {
    let mut files = Vec::new();
    for source in &case.sources {
        let path = directory.join(&source.name);
        if let Some(bytes) = &source.bytes {
            std::fs::write(&path, bytes)?;
        }
        files.push(json!({"path":path,"grfid":source.id,"metadata_version":source.metadata_version,"parameters":source.parameters,"static":source.flags.is_static,"init_only":source.flags.init_only,"system":source.flags.system}));
    }
    Ok(json!({"networking":case.networking,"files":files}))
}
fn context(observation: &Value) -> Result<Vec<u32>> {
    for field in ["arms", "consumptions"] {
        if observation.get(field) != Some(&json!(1)) {
            return Err(format!("invalid {field}").into());
        }
    }
    let before = observation.get("before").ok_or("before context")?;
    let prepared = observation.get("prepared").ok_or("prepared context")?;
    let after = observation.get("after").ok_or("after context")?;
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
        if before.get(field) != prepared.get(field) || prepared.get(field) != after.get(field) {
            return Err(format!("native changed preserved {field}").into());
        }
    }
    let configured: Vec<u32> = before
        .get("configs")
        .and_then(Value::as_array)
        .ok_or("baseline configs")?
        .iter()
        .map(|config| {
            Ok(u32::try_from(
                config
                    .get("grfid")
                    .and_then(Value::as_u64)
                    .ok_or("baseline id")?,
            )?)
        })
        .collect::<Result<_>>()?;
    let paths = observation
        .get("baseline_sources")
        .and_then(Value::as_array)
        .ok_or("baseline source paths")?;
    let scanned = paths
        .iter()
        .map(|path| {
            let bytes = std::fs::read(path.as_str().ok_or("baseline path")?)?;
            Ok(scan_file(&bytes)?
                .metadata
                .ok_or("baseline Action8 metadata")?
                .grfid)
        })
        .collect::<Result<Vec<_>>>()?;
    if configured != scanned {
        return Err("scanned baseline identity mismatch".into());
    }
    Ok(scanned)
}
fn run(root: &Path, directory: &Path, case: &Case) -> Result {
    std::fs::create_dir(directory)?;
    let manifest_path = directory.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest(case, directory)?)?,
    )?;
    let output = Command::new("cmake")
        .arg(format!(
            "-DORACLE={}",
            std::env::var("OTTD_GRF_CONTROL_ORACLE")?
        ))
        .arg(format!("-DRUN_DIR={}", directory.join("native").display()))
        .arg(format!("-DMANIFEST={}", manifest_path.display()))
        .arg(format!(
            "-DINPUT={}",
            root.join("fixtures/replay/clear-v362.sav").display()
        ))
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
        return Err(format!("native {} failed", case.name).into());
    }
    for source in &case.sources {
        if let Some(original) = &source.bytes {
            if std::fs::read(directory.join(&source.name))? != *original {
                return Err("native modified physical GRF bytes".into());
            }
        }
    }
    let raw: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/control.json"))?)?;
    let prefix = context(&raw)?;
    let inputs = case
        .sources
        .iter()
        .map(grf_control_cases::Source::input)
        .collect::<Vec<_>>();
    let report = run_control_load_with_prefix(
        &inputs,
        &prefix,
        ControlOptions {
            networking: case.networking,
            ..ControlOptions::default()
        },
    )?;
    let rust = project::rust(&report)?;
    let native = project::native(&raw, &prefix)?;
    std::fs::write(directory.join("rust.json"), serde_json::to_vec(&rust)?)?;
    std::fs::write(
        directory.join("native-normalized.json"),
        serde_json::to_vec(&native)?,
    )?;
    project::compare(&native, &rust).map_err(|error| format!("{}: {error}", case.name))?;
    controls(directory, &native, &rust)?;
    Ok(())
}
fn controls(directory: &Path, native: &Value, rust: &Value) -> Result {
    let mut controls = Vec::new();
    for name in [
        "stage",
        "file",
        "line",
        "offset",
        "action",
        "executed",
        "skip",
        "next_line",
        "next_offset",
        "config_grfid",
        "file_grfid",
        "version",
        "status",
        "reserved",
        "parameters",
        "labels",
        "errors",
        "bytes",
        "overrides",
    ] {
        let Some(pointer) = project::field_pointer(rust, name, "") else {
            continue;
        };
        let mut wrong = rust.clone();
        let target = wrong.pointer_mut(&pointer).ok_or("negative pointer")?;
        let before = target.clone();
        *target = project::changed(target);
        let after = target.clone();
        if project::compare(native, &wrong).is_ok() {
            return Err("comparator accepted altered control".into());
        }
        controls.push(json!({"field":name,"pointer":pointer,"before":before,"after":after,"comparator_rejected":true}));
    }
    let mut wrong = rust.clone();
    wrong
        .get_mut("events")
        .and_then(Value::as_array_mut)
        .ok_or("event order control")?
        .swap(0, 1);
    if project::compare(native, &wrong).is_ok() {
        return Err("comparator accepted event reordering".into());
    }
    controls.push(json!({"operation":"swap","first":"/events/0","second":"/events/1","comparator_rejected":true}));
    if let Some(base) = project::field_pointer(rust, "errors", "") {
        let pointer = format!("{base}/0/line");
        let mut wrong = rust.clone();
        if let Some(target) = wrong.pointer_mut(&pointer) {
            let before = target.clone();
            *target = project::changed(target);
            let after = target.clone();
            if project::compare(native, &wrong).is_ok() {
                return Err("comparator accepted altered native diagnostic line".into());
            }
            controls.push(json!({"field":"diagnostic-line","pointer":pointer,"before":before,"after":after,"comparator_rejected":true}));
        }
    }
    std::fs::write(
        directory.join("negative-controls.json"),
        serde_json::to_vec(&controls)?,
    )?;
    std::fs::write(
        directory.join("comparison.txt"),
        format!(
            "PASS exact original stage/record/registry/override trace and {} altered comparator controls (recorded as exact JSON substitutions against rust.json)\n",
            controls.len()
        ),
    )?;
    Ok(())
}
#[test]
#[ignore = "requires the original whole-loader observer and a fresh absolute artifact directory"]
fn native_load_control_matrix() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_CONTROL_DIR")?);
    if !directory.is_absolute() {
        return Err("artifact directory must be absolute".into());
    }
    std::fs::create_dir(&directory)?;
    let mut count = 0_u32;
    for case in grf_control_cases::all()? {
        if directory.join("stop-after-case").exists() {
            return Err("matrix stopped at caller-requested case boundary".into());
        }
        if std::env::var("OTTD_GRF_CONTROL_CASE").is_ok_and(|name| name != case.name) {
            continue;
        }
        run(&root, &directory.join(&case.name), &case)?;
        count = count.saturating_add(1);
    }
    if count == 0 {
        return Err("no selected native cases".into());
    }
    println!("PASS {count} native whole-loader cases");
    Ok(())
}
