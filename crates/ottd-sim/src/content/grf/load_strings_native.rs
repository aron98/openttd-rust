use super::{
    language_pack::{BuiltinPack, Pack},
    load_context_tests::native::project,
    load_strings::{Definition, StringKey, StringTable},
    text::Budget,
    text_mapped::TextContext,
    text_translate::{Input, translate_in},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
#[path = "load_strings_cases.rs"]
mod cases;
use cases::Operation;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn compact(table: &StringTable, selected: u8) -> Value {
    json!({"selected":selected,"entries":table.entries.iter().map(|entry| {
        json!([entry.key.grfid,entry.key.local_id,entry.default_id,entry.translations])
    }).collect::<Vec<_>>()})
}

fn populate(
    table: &mut StringTable,
    count: u32,
    grfid: u32,
    raw: &[u8],
    default_id: u32,
    language: u8,
) -> Result {
    if !table.entries.is_empty() || count > 524_288 {
        return Err("invalid declared table fixture".into());
    }
    for local_id in 0..count {
        table.define(
            Definition {
                key: StringKey { grfid, local_id },
                language,
                new_scheme: true,
                default_id,
            },
            |_, _| Ok::<_, std::io::Error>(raw.to_vec()),
        )?;
    }
    Ok(())
}

fn expected(operations: &[Operation], source: &[u8]) -> Result<Value> {
    let pack = Pack::header(source)?;
    let builtins = BuiltinPack::new(source)?;
    let mut selected = pack.language;
    let mut table = StringTable::default();
    let mut budget = Budget::new(super::ScanLimits::default());
    let mut output = Vec::new();
    for operation in operations {
        let mut result = serde_json::Map::new();
        match operation {
            Operation::Define {
                grfid,
                local_id,
                language,
                new_scheme,
                newlines,
                raw,
                default_id,
            } => {
                let id = table.define(
                    Definition {
                        key: StringKey {
                            grfid: *grfid,
                            local_id: *local_id,
                        },
                        language: *language,
                        new_scheme: *new_scheme,
                        default_id: *default_id,
                    },
                    |strings, _| {
                        translate_in(
                            Input {
                                raw,
                                newlines: *newlines,
                                offset: 0,
                                context: TextContext {
                                    map: None,
                                    genders: pack.gender_count,
                                    cases: pack.case_count,
                                },
                            },
                            &mut budget,
                            |local| Some(strings.inline_id(*grfid, local)),
                        )
                    },
                )?;
                result.insert("operation".into(), json!("define"));
                result.insert("id".into(), json!(id));
            }
            Operation::Lookup { grfid, local_id } => {
                result.insert("operation".into(), json!("lookup"));
                result.insert(
                    "id".into(),
                    json!(table.lookup(StringKey {
                        grfid: *grfid,
                        local_id: *local_id
                    })),
                );
            }
            Operation::Select { language } => {
                selected = *language;
                result.insert("operation".into(), json!("select"));
            }
            Operation::Read { id } => {
                result.insert("operation".into(), json!("read"));
                result.insert(
                    "bytes".into(),
                    json!(table.resolve(*id, selected, |id| builtins.lookup(id))?),
                );
            }
            Operation::Reset => {
                table = StringTable::default();
                result.insert("operation".into(), json!("reset"));
            }
            Operation::CapacityFixture {
                count,
                grfid,
                raw,
                default_id,
                language,
            } => {
                populate(&mut table, *count, *grfid, raw, *default_id, *language)?;
                result.insert("operation".into(), json!("capacity_fixture"));
                result.insert(
                    "test_only_declared_table".into(),
                    serde_json::to_value(operation)?,
                );
            }
        }
        result.insert("table".into(), compact(&table, selected));
        output.push(Value::Object(result));
    }
    Ok(Value::Array(output))
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
        format!("-DCONFIG={}", root.join("scripts/reference.cfg").display()),
        "-P".into(),
        root.join("scripts/check-grf-strings-reference.cmake")
            .display()
            .to_string(),
    ];
    std::fs::write(directory.join("argv.json"), serde_json::to_vec(&args)?)?;
    let process = Command::new("cmake").args(&args).output()?;
    std::fs::write(directory.join("stdout.log"), &process.stdout)?;
    std::fs::write(directory.join("stderr.log"), &process.stderr)?;
    std::fs::write(
        directory.join("status.json"),
        serde_json::to_vec(&process.status.code())?,
    )?;
    if !process.status.success() {
        return Err("original strings witness failed".into());
    }
    Ok(serde_json::from_slice(&std::fs::read(
        directory.join("native/strings.json"),
    )?)?)
}

pub(super) fn checked(native: &Value, rust: &mut Value, directory: &Path) -> Result<usize> {
    project::compare(native, rust)?;
    let rows = rust.as_array().ok_or("results array")?.len();
    let mut controls = Vec::new();
    for row in 0..rows {
        for suffix in [
            "stage",
            "file",
            "line",
            "offset",
            "translation_errors/0/file",
            "translation_errors/0/line",
            "translation_errors/0/message",
            "translation_errors/0/severity",
            "translation_errors/0/parameters/0",
            "translation_errors/0/parameters/1",
            "translation_errors/0/data",
            "translation_errors/0/custom_message",
            "operation",
            "id",
            "bytes",
            "table/selected",
            "table/entries/0/0",
            "table/entries/0/1",
            "table/entries/0/2",
            "table/entries/0/3/0/0",
            "table/entries/0/3/0/1",
        ] {
            let pointer = format!("/{row}/{suffix}");
            let Some(original) = rust.pointer(&pointer).cloned() else {
                continue;
            };
            let changed = match &original {
                Value::Number(n) => json!(n.as_u64().ok_or("unsigned observable")?.wrapping_add(1)),
                Value::String(s) => json!(format!("{s}-altered")),
                Value::Array(values) => {
                    let mut changed = values.clone();
                    changed.push(json!(255));
                    json!(changed)
                }
                Value::Null | Value::Bool(_) | Value::Object(_) => {
                    return Err("unexpected mutation domain".into());
                }
            };
            *rust.pointer_mut(&pointer).ok_or("mutation target")? = changed.clone();
            let rejection = project::compare(
                native.get(row).ok_or("native observation")?,
                rust.get(row).ok_or("Rust observation")?,
            )
            .is_err();
            *rust.pointer_mut(&pointer).ok_or("restore target")? = original;
            if !rejection {
                return Err(format!("accepted altered {pointer}").into());
            }
            controls.push(json!({"pointer":pointer,"altered":changed,"rejected":true}));
        }
    }
    if let Some(last) = rust.as_array_mut().ok_or("results array")?.pop() {
        let rejected = project::compare(native, rust).is_err();
        rust.as_array_mut().ok_or("results array")?.push(last);
        if !rejected {
            return Err("accepted missing final observation".into());
        }
        controls.push(json!({"operation":"remove-final-observation","rejected":true}));
    }
    let extra = rust.as_array().ok_or("results array")?.last().cloned();
    if let Some(extra) = extra {
        rust.as_array_mut().ok_or("results array")?.push(extra);
        let rejected = project::compare(native, rust).is_err();
        rust.as_array_mut().ok_or("results array")?.pop();
        if !rejected {
            return Err("accepted extra observation".into());
        }
        controls.push(json!({"operation":"extra-observation","rejected":true}));
    }
    if let Some(other) = (1..rows).find(|&index| rust.get(index) != rust.get(0)) {
        rust.as_array_mut().ok_or("results array")?.swap(0, other);
        let rejected = project::compare(native, rust).is_err();
        rust.as_array_mut().ok_or("results array")?.swap(0, other);
        if !rejected {
            return Err("accepted reordered observations".into());
        }
        controls.push(json!({"operation":"reorder-observations","other":other,"rejected":true}));
    }
    std::fs::write(
        directory.join("controls.json"),
        serde_json::to_vec(&controls)?,
    )?;
    Ok(controls.len())
}

#[test]
#[ignore = "requires original strings observer and fresh absolute artifact directory"]
fn original_string_api_matrix() -> Result {
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
    let source_path = Path::new(&oracle)
        .parent()
        .ok_or("oracle parent")?
        .join("lang/english.lng");
    let source = std::fs::read(&source_path)?;
    let language = Pack::header(&source)?.language;
    let mut summary = Vec::new();
    for case in cases::cases() {
        let case_directory = directory.join(case.name);
        std::fs::create_dir(&case_directory)?;
        let pack_directory = case_directory.join("pack");
        std::fs::create_dir(&pack_directory)?;
        std::fs::write(pack_directory.join("input.lng"), &source)?;
        let manifest = json!({"files":[],"networking":false,"language":{
            "pack_directories":[pack_directory],"selected":language,"queries":[]},"strings_api":case.operations});
        let native = invoke(&root, &case_directory, &manifest, &oracle)?;
        let mut rust = expected(&case.operations, &source)?;
        std::fs::write(case_directory.join("rust.json"), serde_json::to_vec(&rust)?)?;
        let controls = checked(
            native.get("results").ok_or("native API results")?,
            &mut rust,
            &case_directory,
        )?;
        summary
            .push(json!({"case":case.name,"operations":case.operations.len(),"controls":controls}));
    }
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(&summary)?,
    )?;
    println!("compared {} original string API cases", summary.len());
    Ok(())
}

#[test]
fn comparator_rejects_changed_string_identity() -> Result {
    let native = json!([{"operation":"lookup","id":2,"table":{"selected":1,"entries":[]}}]);
    let mut changed = native.clone();
    *changed.pointer_mut("/0/id").ok_or("id")? = json!(3);
    assert!(project::compare(&native, &changed).is_err());
    Ok(())
}
