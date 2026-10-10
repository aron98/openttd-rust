use super::load_context_tests::native::project;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
pub(super) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) fn setup() -> Result<(PathBuf, PathBuf, String, Vec<u8>)> {
    for name in [
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_CURRENCY_CASE",
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
    let directory = PathBuf::from(std::env::var("OTTD_GRF_CURRENCY_ARTIFACTS")?);
    if !directory.is_absolute() {
        return Err("absolute artifacts required".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::copy(std::env::current_exe()?, directory.join("test-executable"))?;
    let oracle = std::env::var("OTTD_GRF_CURRENCY_ORACLE")?;
    let pack = std::fs::read(
        Path::new(&oracle)
            .parent()
            .ok_or("oracle parent")?
            .join("lang/english.lng"),
    )?;
    Ok((root, directory, oracle, pack))
}

pub(super) fn invoke(
    root: &Path,
    directory: &Path,
    manifest: &Value,
    oracle: &str,
) -> Result<Value> {
    let path = directory.join("manifest.json");
    std::fs::write(&path, serde_json::to_vec(manifest)?)?;
    let args = vec![
        format!("-DORACLE={oracle}"),
        format!("-DRUN_DIR={}", directory.join("native").display()),
        format!("-DMANIFEST={}", path.display()),
        format!(
            "-DINPUT={}",
            root.join("fixtures/replay/clear-v362.sav").display()
        ),
        format!("-DCONFIG={}", directory.join("config.cfg").display()),
        "-P".into(),
        root.join("scripts/check-grf-currency-reference.cmake")
            .display()
            .to_string(),
    ];
    let mut config = std::fs::read_to_string(root.join("scripts/reference.cfg"))?;
    config.push_str("\n[currency]\nrate = 1\nseparator = \".\"\nto_euro = 0\nprefix = \"\"\nsuffix = \" credits\"\n");
    std::fs::write(directory.join("config.cfg"), config)?;
    std::fs::write(directory.join("argv.json"), serde_json::to_vec(&args)?)?;
    let process = Command::new("cmake").args(&args).output()?;
    std::fs::write(directory.join("stdout.log"), &process.stdout)?;
    std::fs::write(directory.join("stderr.log"), &process.stderr)?;
    std::fs::write(
        directory.join("status.json"),
        serde_json::to_vec(&process.status.code())?,
    )?;
    if !process.status.success() {
        return Err(format!("original currency witness failed: {}", directory.display()).into());
    }
    Ok(serde_json::from_slice(&std::fs::read(
        directory.join("native/currency.json"),
    )?)?)
}

pub(super) fn compare(native: &Value, rust: &Value, directory: &Path) -> Result<usize> {
    fn visit(native: &Value, rust: &Value, path: &str, controls: &mut Vec<Value>) -> Result {
        match rust {
            Value::Array(values) => {
                for (i, value) in values.iter().enumerate() {
                    visit(
                        native.get(i).ok_or("native array member")?,
                        value,
                        &format!("{path}/{i}"),
                        controls,
                    )?;
                }
                let mut changed = values.clone();
                changed.push(Value::Null);
                if project::compare(native, &json!(changed)).is_ok() {
                    return Err("accepted extra array member".into());
                }
                controls.push(json!({"path":path,"mutation":"extra-array-member","rejected":true}));
            }
            Value::Object(values) => {
                for (key, value) in values {
                    visit(
                        native.get(key).ok_or("native object member")?,
                        value,
                        &format!("{path}/{key}"),
                        controls,
                    )?;
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                let changed = if rust.is_null() {
                    json!(false)
                } else {
                    Value::Null
                };
                if project::compare(native, &changed).is_ok() {
                    return Err("accepted altered scalar".into());
                }
                controls.push(json!({"path":path,"altered":changed,"rejected":true}));
            }
        }
        Ok(())
    }
    std::fs::write(directory.join("rust.json"), serde_json::to_vec(rust)?)?;
    project::compare(native, rust)?;
    let mut controls = Vec::new();
    visit(native, rust, "", &mut controls)?;
    std::fs::write(
        directory.join("controls.json"),
        serde_json::to_vec(&controls)?,
    )?;
    Ok(controls.len())
}
