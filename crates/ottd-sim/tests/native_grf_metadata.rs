//! Full metadata and binary text observations from the pinned original runtime.
/// Shared independently generated metadata scenarios and projections.
pub mod grf_metadata_cases;
use grf_metadata_cases::{Result, cases, file, project, text};
use ottd_sim::content::grf::translate_fresh_text;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn compare(native: &Value, rust: &Value) -> Result {
    if native != rust {
        return Err("native metadata mismatch".into());
    }
    Ok(())
}

fn alter(value: &mut Value) -> Result {
    match value {
        Value::Bool(flag) => *flag = !*flag,
        Value::Number(number) => *number = (number.as_u64().ok_or("unsigned field")? ^ 1).into(),
        Value::Array(bytes) => {
            if let Some(first) = bytes.first_mut() {
                alter(first)?;
            } else {
                bytes.push(json!(0));
            }
        }
        Value::Null => *value = json!([]),
        Value::String(_) | Value::Object(_) => return Err("unexpected sensitivity field".into()),
    }
    Ok(())
}
fn prepare(directory: &Path) -> Result<Value> {
    std::fs::create_dir_all(directory.join("inputs"))?;
    let mut scans = Vec::new();
    let mut projected = Vec::new();
    for (name, actions) in cases()? {
        for version in [1, 2] {
            for palette in [0, 1] {
                let name = format!("{name}-v{version}-p{palette}");
                let bytes = file(&actions, version)?;
                let path = directory
                    .join("inputs")
                    .join(format!("{}.grf", scans.len()));
                std::fs::write(&path, &bytes)?;
                let (result, mut scenario) = project::scan(&bytes, &name, palette)?;
                scenario
                    .as_object_mut()
                    .ok_or("scenario")?
                    .insert("path".into(), json!(path));
                projected.push(result);
                scans.push(scenario);
            }
        }
    }
    let mut texts = Vec::new();
    let mut translated = Vec::new();
    for (name, raw, newlines) in text::direct() {
        translated.push(
            json!({"name":name,"translated":translate_fresh_text(&raw,newlines,8*1024*1024)?}),
        );
        texts.push(json!({"name":name,"raw":raw,"newlines":newlines,"grfid":0,"language":1}));
    }
    let manifest = json!({"scans":scans,"texts":texts});
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec(&manifest)?,
    )?;
    let projected = json!({"scans":projected,"texts":translated});
    std::fs::write(directory.join("rust.json"), serde_json::to_vec(&projected)?)?;
    Ok(projected)
}

#[test]
fn metadata_matrix_inputs_are_admitted() -> Result {
    for (name, actions) in cases()? {
        for version in [1, 2] {
            for palette in [0, 1] {
                project::scan(&file(&actions, version)?, &name, palette)?;
            }
        }
    }
    for (_, raw, newlines) in text::direct() {
        translate_fresh_text(&raw, newlines, 8 * 1024 * 1024)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires original metadata observer; set OTTD_GRF_METADATA_ORACLE and OTTD_GRF_METADATA_DIR"]
fn native_metadata_matrix() -> Result {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = PathBuf::from(std::env::var("OTTD_GRF_METADATA_DIR")?);
    if directory.exists() {
        return Err("fresh directory required".into());
    }
    std::fs::create_dir_all(&directory)?;
    let directory = directory.canonicalize()?;
    let rust = prepare(&directory)?;
    let output = Command::new("cmake")
        .arg(format!(
            "-DORACLE={}",
            std::env::var("OTTD_GRF_METADATA_ORACLE")?
        ))
        .arg(format!("-DRUN_DIR={}", directory.join("native").display()))
        .arg(format!(
            "-DMANIFEST={}",
            directory.join("manifest.json").display()
        ))
        .arg(format!(
            "-DINPUT={}",
            root.join("fixtures/replay/clear-v362.sav").display()
        ))
        .arg(format!(
            "-DCONFIG={}",
            root.join("scripts/reference.cfg").display()
        ))
        .arg("-P")
        .arg(root.join("scripts/check-grf-metadata-reference.cmake"))
        .output()?;
    std::fs::write(directory.join("stdout.log"), &output.stdout)?;
    std::fs::write(directory.join("stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err("native metadata observer failed".into());
    }
    let native: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/metadata.json"))?)?;
    compare(&native, &rust)?;
    let scans = native
        .get("scans")
        .ok_or("scans")?
        .as_array()
        .ok_or("scans")?;
    let mut controls = 0_usize;
    for (index, scan) in scans.iter().enumerate() {
        for pointer in [
            "/metadata/palette_bits",
            "/metadata/name/entries/0/translated",
            "/metadata/parameters/0/default",
            "/metadata/parameters/0/complete_labels",
            "/defaults/parameters/0",
            "/metadata/name/selections/0/text",
            "/metadata/name/selections/0/language",
            "/metadata/compatibility/0/compatible",
            "/failure/line",
        ] {
            let mut altered = scan.clone();
            if let Some(field) = altered.pointer_mut(pointer) {
                alter(field)?;
                assert!(compare(scan, &altered).is_err());
                controls = controls.saturating_add(1);
                std::fs::write(
                    directory.join(format!("negative-{index}-{controls}.json")),
                    serde_json::to_vec(&altered)?,
                )?;
            }
        }
    }
    std::fs::write(
        directory.join("comparison.txt"),
        format!(
            "PASS scans={} texts={} comparator_rejections={controls}\n",
            scans.len(),
            native
                .get("texts")
                .ok_or("texts")?
                .as_array()
                .ok_or("texts")?
                .len()
        ),
    )?;
    Ok(())
}
