use super::{
    ControlOptions,
    language_pack::Pack,
    load::{RuntimeInputs, run_with_currency},
    load_context_tests::{grf_control_cases as programs, native::project},
    load_currency::{CurrencyOwner, CurrencyState},
    load_currency_native::{Result, compare, invoke, setup},
    load_language_state::{LanguageInput, LanguageLimits, LanguageReport},
};
use serde_json::{Value, json};
use std::path::Path;
#[path = "load_currency_load_cases.rs"]
mod expanded;

fn property(first: u16, ids: &[u16]) -> Result<Vec<u8>> {
    let mut bytes = vec![0, 8, 1, u8::try_from(ids.len())?, 255];
    bytes.extend(first.to_le_bytes());
    bytes.push(0x0a);
    bytes.extend(ids.iter().flat_map(|id| id.to_le_bytes()));
    Ok(bytes)
}
fn define(local: u16, byte: u8) -> Vec<u8> {
    let mut bytes = vec![4, 0, 0xff, 1];
    bytes.extend(local.to_le_bytes());
    bytes.extend([byte, 0]);
    bytes
}
type Case = (&'static str, Vec<programs::Source>, Option<Vec<usize>>);
fn cases() -> Result<Vec<Case>> {
    let first = || {
        programs::source(
            7,
            &[
                property(0, &[0xd800])?,
                property(31, &[0xd800])?,
                define(0xd800, b'A'),
            ],
            &[],
            1,
        )
    };
    let second = || programs::source(8, &[property(0, &[0xd801])?, define(0xd801, b'B')], &[], 2);
    Ok(vec![
        ("deferred-and-overwrite", vec![first()?, second()?], None),
        ("reload-empty-custom-retained", vec![first()?], Some(vec![])),
        (
            "reload-reordered",
            vec![first()?, second()?],
            Some(vec![1, 0]),
        ),
        (
            "disabled-retained",
            vec![
                programs::source(
                    7,
                    &[
                        property(0, &[0xd800])?,
                        define(0xd800, b'A'),
                        vec![0x13, 8, 0, 0, 0],
                    ],
                    &[],
                    1,
                )?,
                programs::source(8, &[], &[], 1)?,
            ],
            None,
        ),
        (
            "wrapped-and-invalid",
            vec![programs::source(
                7,
                &[
                    property(255, &[0, 0xd800])?,
                    property(46, &[0xd800])?,
                    define(0xd800, b'A'),
                ],
                &[],
                2,
            )?],
            None,
        ),
    ])
}
fn table(entries: &[super::load_strings::Entry], selected: u8) -> Value {
    json!({"selected":selected,"entries":entries.iter().map(|entry|json!([entry.key.grfid,entry.key.local_id,entry.default_id,entry.translations])).collect::<Vec<_>>()})
}
fn state(currency: &CurrencyState, strings: &Value) -> Value {
    json!({"owners":currency.owners.entries,"pending":currency.pending.iter().map(|p|json!({"grfid":p.grfid,"source":p.source})).collect::<Vec<_>>(),"strings":strings})
}
fn native_rows(load: &Value, prefix: usize) -> Result<Value> {
    let mut rows = Vec::new();
    for event in load
        .get("events")
        .and_then(Value::as_array)
        .ok_or("native events")?
    {
        let phase = event
            .get("phase")
            .and_then(Value::as_str)
            .ok_or("event phase")?;
        let coordinates = match phase {
            "decision" => {
                let record = event.get("record").ok_or("record")?;
                let file =
                    usize::try_from(record.get("file").and_then(Value::as_u64).ok_or("file")?)?;
                if file < prefix {
                    continue;
                }
                json!({"stage":record.get("stage"),"file":file.saturating_sub(prefix),"line":record.get("line"),"offset":record.get("offset")})
            }
            "stage-end" => json!({"stage":event.get("stage"),"file":0,"line":0,"offset":0}),
            "before-reset" | "after-reset" | "mapping-added" | "mapping-applied"
            | "before-finalize" | "after-finalize" | "finish" => continue,
            _ => return Err(format!("unknown native phase {phase}").into()),
        };
        rows.push(json!({"coordinates":coordinates,"state":{"owners":event.get("owners"),"pending":event.get("pending"),"strings":event.get("strings")}}));
    }
    Ok(json!(rows))
}
fn rust_rows(report: &LanguageReport, custom: Option<&CurrencyOwner>) -> Value {
    let initial = custom.map_or_else(CurrencyState::default, CurrencyState::with_custom);
    json!(report.events.iter().map(|event|json!({"coordinates":{"stage":event.stage,"file":event.file,"line":event.line,"offset":event.offset},
        "state":state(event.currency.as_ref().unwrap_or(&initial),&table(&event.strings,report.selected))})).collect::<Vec<_>>())
}

fn configured_custom() -> Result<CurrencyOwner> {
    // Original table/settings/currency_settings.ini settings are NotInSave.
    // The exact same values are declared in this witness's config.cfg.
    let mut custom = CurrencyState::default()
        .owners
        .entries
        .get(31)
        .ok_or("custom default")?
        .clone();
    custom.separator = ".".into();
    custom.suffix = " credits".into();
    Ok(custom)
}

fn compare_lifecycle(
    load: &Value,
    language: &LanguageReport,
    custom: Option<&CurrencyOwner>,
    prefix: &[u32],
    control: &super::ControlLoadReport,
    directory: &Path,
) -> Result {
    let initial = custom.map_or_else(CurrencyState::default, CurrencyState::with_custom);
    let before = language
        .events
        .last()
        .and_then(|event| event.currency.as_ref())
        .unwrap_or(&initial);
    let strings = table(&language.strings, language.selected);
    let mut observations = Vec::new();
    let mut originals = Vec::new();
    let mut applied = 0_usize;
    let mut string_table = super::load_strings::StringTable::default();
    for entry in &language.strings {
        for (language, bytes) in &entry.translations {
            string_table.define(
                super::load_strings::Definition {
                    key: entry.key,
                    language: *language,
                    new_scheme: true,
                    default_id: entry.default_id,
                },
                |_, _| Ok::<_, std::io::Error>(bytes.clone()),
            )?;
        }
    }
    for event in load
        .get("events")
        .and_then(Value::as_array)
        .ok_or("lifecycle events")?
    {
        let phase = event.get("phase").and_then(Value::as_str).ok_or("phase")?;
        let expected = match phase {
            "after-reset" => state(&initial, &table(&[], language.selected)),
            "before-finalize" => state(before, &strings),
            "mapping-applied" => {
                applied = applied.saturating_add(1);
                let mut partial = before.clone();
                partial.pending.truncate(applied);
                partial.finalize(&string_table);
                partial.pending = before.pending.clone();
                state(&partial, &strings)
            }
            "after-finalize" | "finish" => {
                state(language.currency.as_ref().unwrap_or(&initial), &strings)
            }
            "before-reset" | "decision" | "mapping-added" | "stage-end" => continue,
            _ => return Err("unknown lifecycle phase".into()),
        };
        observations.push(json!({"phase":phase,"state":expected}));
        originals.push(json!({"phase":phase,"state":{"owners":event.get("owners"),"pending":event.get("pending"),"strings":event.get("strings")}}));
    }
    if applied != before.pending.len() {
        return Err("mapping callback count mismatch".into());
    }
    let lifecycle = directory.join("lifecycle");
    std::fs::create_dir(&lifecycle)?;
    compare(&json!(originals), &json!(observations), &lifecycle)?;
    let original = project::native(
        &json!({"events":[],"files":load.get("files"),"overrides":[]}),
        prefix,
    )?;
    let rust = project::rust(control)?;
    project::compare(
        original.get("files").ok_or("native final files")?,
        rust.get("files").ok_or("Rust final files")?,
    )?;
    Ok(())
}

fn baseline_ids(native: &Value) -> Result<Vec<u32>> {
    native
        .get("baseline_sources")
        .and_then(Value::as_array)
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

fn run_case(
    root: &Path,
    directory: &Path,
    oracle: &str,
    pack: &[u8],
    sources: &[programs::Source],
    reload: Option<&[usize]>,
) -> Result<Value> {
    std::fs::create_dir(directory)?;
    let packs_dir = directory.join("pack");
    std::fs::create_dir(&packs_dir)?;
    std::fs::write(packs_dir.join("input.lng"), pack)?;
    let selected = Pack::header(pack)?.language;
    let mut names = Vec::new();
    let mut files = Vec::new();
    for source in sources {
        let path = directory.join(&source.name);
        if let Some(bytes) = &source.bytes {
            std::fs::write(&path, bytes)?;
        }
        names.push(path.display().to_string());
        files.push(json!({"path":path,"grfid":source.id,"metadata_version":source.metadata_version,"parameters":source.parameters,"static":source.flags.is_static,"init_only":source.flags.init_only,"system":source.flags.system,"palette":1}));
    }
    let mut manifest = json!({"files":files,"networking":false,"currency_load":true,"language":{"pack_directories":[packs_dir],"selected":selected,"queries":[]}});
    if let Some(order) = reload {
        manifest
            .as_object_mut()
            .ok_or("manifest object")?
            .insert("currency_reload".into(), json!(order));
    }
    let native = invoke(root, directory, &manifest, oracle)?;
    let prefix = baseline_ids(&native)?;
    let mut orders = vec![(0..sources.len()).collect::<Vec<_>>()];
    if let Some(order) = reload {
        orders.push(order.to_vec());
    }
    let mut previous_custom = Some(configured_custom()?);
    let mut summary = Vec::new();
    for (number, order) in orders.iter().enumerate() {
        let inputs = order
            .iter()
            .map(|index| {
                let source = sources.get(*index).ok_or("input index")?;
                let mut input = source.input();
                input.name = names.get(*index).ok_or("name index")?;
                Ok(input)
            })
            .collect::<Result<Vec<_>>>()?;
        let packs = [pack];
        let (control, _, language) = run_with_currency(
            &inputs,
            &prefix,
            ControlOptions::default(),
            RuntimeInputs {
                environment: None,
                language: Some(LanguageInput {
                    packs: &packs,
                    selected,
                    limits: LanguageLimits::default(),
                }),
            },
            previous_custom.as_ref(),
        )?;
        let language = language.ok_or("language")?;
        let load = native
            .get("loads")
            .and_then(|loads| loads.get(number))
            .ok_or("native load")?;
        let output = directory.join(format!("load-{number}"));
        std::fs::create_dir(&output)?;
        let controls = compare(
            &native_rows(load, prefix.len())?,
            &rust_rows(&language, previous_custom.as_ref()),
            &output,
        )?;
        compare_lifecycle(
            load,
            &language,
            previous_custom.as_ref(),
            &prefix,
            &control,
            &output,
        )?;
        let fallback = previous_custom
            .as_ref()
            .map_or_else(CurrencyState::default, CurrencyState::with_custom);
        let currency = language.currency.as_ref().unwrap_or(&fallback);
        let rust_final = state(currency, &table(&language.strings, selected));
        project::compare(
            &json!({"owners":load.get("owners"),"pending":load.get("pending"),"strings":load.get("strings")}),
            &rust_final,
        )?;
        std::fs::write(
            output.join("rust-final.json"),
            serde_json::to_vec(&rust_final)?,
        )?;
        std::fs::write(
            output.join("rust-control.json"),
            serde_json::to_vec(&project::rust(&control)?)?,
        )?;
        previous_custom = currency.owners.entries.get(31).cloned();
        summary.push(json!({"load":number,"events":language.events.len(),"controls":controls}));
    }
    Ok(json!(summary))
}

#[test]
#[ignore = "requires actual original currency scheduler and fresh absolute artifacts"]
fn original_currency_load_matrix() -> Result {
    run_cases(&expanded::cases()?)
}

#[test]
#[ignore = "bounded pilot: one original load then one two-load process, not full matrix"]
fn original_currency_load_pilot() -> Result {
    run_cases(cases()?.get(..2).ok_or("pilot cases")?)
}

#[test]
#[ignore = "remaining three pilot cases, not expanded currency admission"]
fn original_currency_remaining_pilot() -> Result {
    run_cases(cases()?.get(2..).ok_or("remaining pilot cases")?)
}

fn run_cases(cases: &[Case]) -> Result {
    let (root, directory, oracle, pack) = setup()?;
    let mut summary = Vec::new();
    for (name, sources, reload) in cases {
        summary.push(json!({"case":name,"loads":run_case(&root,&directory.join(name),&oracle,&pack,sources,reload.as_deref())?}));
    }
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(&summary)?,
    )?;
    println!("compared {} original currency load cases", summary.len());
    Ok(())
}
