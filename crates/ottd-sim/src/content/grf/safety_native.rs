use super::{
    SafetyLimits, SafetyOutcome, ScanFailure, ScanOptions, ScanStatus, safety_cases, scan_file,
    scan_static_file,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn project(case: &safety_cases::Case) -> Result<Value> {
    let (scan, safety) = if case.is_static {
        let report = scan_static_file(
            &case.bytes,
            ScanOptions::default(),
            &case.configs,
            SafetyLimits::default(),
        )?;
        (report.scan, report.safety)
    } else {
        (scan_file(&case.bytes)?, None)
    };
    let identity = scan.identity.map_or(
        Value::Null,
        |identity| json!({"grfid":identity.grfid,"md5":identity.md5}),
    );
    let failure = scan.failure.map_or(Value::Null, |(reason,line)| json!({"reason":match reason { ScanFailure::ReadBounds=>"ReadBounds", ScanFailure::UnexpectedSprite=>"UnexpectedSprite" },"line":line}));
    let decisions: Vec<_> = safety.as_ref().map_or(&[][..], |s| &s.decisions).iter().map(|d|json!({"line":d.line,"offset":d.offset,"action":d.action,"consumed":d.consumed,"skip":d.skip})).collect();
    Ok(
        json!({"id":case.id,"accepted":scan.accepted,"identity":identity,"grfid":scan.metadata.as_ref().map_or(0,|m|m.grfid),
        "status":match scan.status {ScanStatus::Unknown=>0,ScanStatus::Disabled=>1},
        "unsafe":safety.as_ref().is_some_and(|s|matches!(s.outcome,SafetyOutcome::Unsafe {..})),"system":scan.system,"invalid":scan.invalid_version,
        "name":scan.static_info.name.select(1).map(|text|&text.translated),"info":scan.static_info.description.select(1).map(|text|&text.translated),
        "failure":failure,"decisions":decisions}),
    )
}
fn compare(native: &Value, rust: &Value) -> Result {
    if native != rust {
        return Err("static safety native mismatch".into());
    }
    Ok(())
}

#[test]
fn generated_safety_cases_have_unique_names_and_project() -> Result {
    let cases = safety_cases::cases()?;
    let mut ids = std::collections::BTreeSet::new();
    for case in cases {
        assert!(ids.insert(case.id.clone()), "duplicate {}", case.id);
        project(&case)?;
    }
    assert!(ids.len() > 800);
    Ok(())
}

#[test]
#[ignore = "requires original native safety observer"]
fn compare_static_safety_with_original() -> Result {
    for key in [
        "OTTD_GRF_SAFETY_CASE",
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_REPLAY_PATH",
    ] {
        if std::env::var_os(key).is_some() {
            return Err(format!("forbidden subset/replay {key}").into());
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_SAFETY_ARTIFACTS")?);
    if !directory.is_absolute() {
        return Err("absolute artifact path required".into());
    }
    std::fs::create_dir(&directory)?;
    let cases = safety_cases::cases()?;
    let mut manifest = Vec::new();
    for case in &cases {
        let path = directory.join(format!("{}.grf", case.id));
        std::fs::write(&path, &case.bytes)?;
        let configs: Vec<_> = case
            .configs
            .iter()
            .map(|c| json!({"grfid":c.grfid,"is_static":c.is_static}))
            .collect();
        manifest
            .push(json!({"id":case.id,"path":path,"is_static":case.is_static,"configs":configs}));
    }
    let manifest_path = directory.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
    let output = Command::new("cmake")
        .arg(format!("-DORACLE={}", std::env::var("OTTD_GRF_ORACLE")?))
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
        .arg(root.join("scripts/check-grf-safety-reference.cmake"))
        .output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err("native safety runner failed".into());
    }
    verify_results(&directory, &cases)
}

fn verify_results(directory: &Path, cases: &[safety_cases::Case]) -> Result {
    let raw: Value = serde_json::from_slice(&std::fs::read(directory.join("native/safety.json"))?)?;
    if raw.get("before") != raw.get("restored")
        || raw.get("files_before") != raw.get("files_restored")
    {
        return Err("native fixture restoration failed".into());
    }
    let native = raw
        .get("scans")
        .and_then(Value::as_array)
        .ok_or("native scans")?;
    if native.len() != cases.len() {
        return Err("native case count".into());
    }
    let mut controls = 0_u64;
    let mut decision_count = 0_usize;
    for (case, observed) in cases.iter().zip(native) {
        let rust = project(case)?;
        let case_dir = directory.join(&case.id);
        std::fs::create_dir(&case_dir)?;
        std::fs::write(case_dir.join("native.json"), serde_json::to_vec(observed)?)?;
        std::fs::write(case_dir.join("rust.json"), serde_json::to_vec(&rust)?)?;
        compare(observed, &rust).map_err(|e| format!("{e}: {}", case.id))?;
        for pointer in [
            "/id",
            "/accepted",
            "/identity",
            "/status",
            "/unsafe",
            "/failure",
            "/decisions",
            "/decisions/0/offset",
            "/decisions/0/consumed",
            "/decisions/0/skip",
            "/decisions/1/action",
            "/decisions/1/consumed",
            "/decisions/1/skip",
            "/name",
            "/info",
            "/identity/md5",
        ] {
            let mut wrong = rust.clone();
            if let Some(value) = wrong.pointer_mut(pointer) {
                *value = match value {
                    Value::Bool(flag) => json!(!*flag),
                    Value::Number(number) => json!(
                        number
                            .as_i64()
                            .ok_or("integer control")?
                            .checked_add(1)
                            .ok_or("control overflow")?
                    ),
                    Value::Array(array) => {
                        if array.is_empty() {
                            json!([1])
                        } else {
                            json!([])
                        }
                    }
                    Value::String(text) => json!(format!("{text}-corrupt")),
                    Value::Null | Value::Object(_) => json!("deliberate corruption"),
                };
                assert!(compare(observed, &wrong).is_err());
                controls = controls.checked_add(1).ok_or("control count")?;
                std::fs::write(
                    case_dir.join(format!("negative-{controls}.json")),
                    serde_json::to_vec(&wrong)?,
                )?;
            }
        }
        decision_count = decision_count
            .checked_add(
                rust.get("decisions")
                    .and_then(Value::as_array)
                    .ok_or("Rust decisions")?
                    .len(),
            )
            .ok_or("decision count")?;
    }
    std::fs::write(
        directory.join("coverage.json"),
        serde_json::to_vec(
            &json!({"cases":cases.len(),"ids":cases.iter().map(|case|&case.id).collect::<Vec<_>>(),"decisions":decision_count,"comparator_rejections":controls}),
        )?,
    )?;
    Ok(())
}
