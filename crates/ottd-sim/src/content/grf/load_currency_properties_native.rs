use super::{
    language_pack::Pack,
    load_context_tests::native::project,
    load_currency::{CurrencyOwners, CurrencyState},
    load_currency_native::{Result, compare, invoke, setup},
    load_currency_properties::Property,
    load_strings::StringTable,
    records::Reader,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    currency_owner_encoding: String,
    currency_api: Vec<Operation>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Operation {
    Property {
        grfid: u32,
        property: u8,
        first: u16,
        count: u8,
        reserve: bool,
        raw: Vec<u8>,
    },
    ResetCurrencies {
        preserve_custom: bool,
    },
}

fn expected(operations: &[Operation], selected: u8) -> Result<Value> {
    let mut currency = CurrencyState::default();
    let strings = StringTable::default();
    let mut rows = Vec::new();
    for operation in operations {
        let mut row = serde_json::Map::new();
        match operation {
            Operation::Property {
                property,
                first,
                count,
                reserve,
                raw,
                ..
            } => {
                row.insert("operation".into(), json!("property"));
                row.insert("property".into(), json!(property));
                let property = Property::from_id(*property).ok_or("fixture property")?;
                let mut reader = Reader { bytes: raw, pos: 0 };
                let mut bounds = false;
                for index in u32::from(*first)..u32::from(*first).saturating_add(u32::from(*count))
                {
                    match property.read(&mut reader, !*reserve) {
                        Ok(Some(value)) => value.apply(&mut currency, index),
                        Ok(None) => {}
                        Err(_) => {
                            bounds = true;
                            break;
                        }
                    }
                }
                if bounds {
                    row.insert("read_bounds".into(), json!(true));
                } else {
                    row.insert("result".into(), json!(0));
                }
                row.insert("remaining".into(), json!(reader.remaining()));
            }
            Operation::ResetCurrencies { preserve_custom } => {
                row.insert("operation".into(), json!("reset_currencies"));
                currency.owners = CurrencyOwners::reset(if *preserve_custom {
                    currency.owners.entries.get(31)
                } else {
                    None
                });
            }
        }
        row.insert("owners".into(), json!(currency.owners.entries));
        row.insert(
            "pending".into(),
            json!(
                currency
                    .pending
                    .iter()
                    .map(|p| json!({"grfid":p.grfid,"source":p.source}))
                    .collect::<Vec<_>>()
            ),
        );
        row.insert(
            "strings".into(),
            json!({"selected":selected,"entries":strings.entries}),
        );
        rows.push(row);
    }
    Ok(json!(rows))
}

#[test]
#[ignore = "requires native25 byte observer and fresh absolute currency artifacts"]
fn original_currency_property_pilot() -> Result {
    let fixture: Fixture =
        serde_json::from_str(include_str!("load_currency_properties_pilot.json"))?;
    if fixture.currency_owner_encoding != "bytes-v1" || fixture.currency_api.len() != 9 {
        return Err("unexpected property pilot fixture".into());
    }
    let (root, directory, oracle, pack) = setup()?;
    let controls = compare_api(&root, &directory, &oracle, &pack, &fixture.currency_api)?;
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(
            &json!({"case":"nine-operation-byte-pilot","operations":fixture.currency_api.len(),"controls":controls,"encoding":"bytes-v1"}),
        )?,
    )?;
    println!("compared 9 original currency property operations with {controls} rejection controls");
    Ok(())
}

pub(super) fn compare_api(
    root: &Path,
    directory: &Path,
    oracle: &str,
    pack: &[u8],
    operations: &[Operation],
) -> Result<usize> {
    let selected = Pack::header(pack)?.language;
    let packs = directory.join("pack");
    std::fs::create_dir(&packs)?;
    std::fs::write(packs.join("input.lng"), pack)?;
    let manifest = json!({"files":[],"networking":false,"currency_owner_encoding":"bytes-v1",
        "currency_api":operations,"language":{"pack_directories":[packs],"selected":selected,"queries":[]}});
    let native = invoke(root, directory, &manifest, oracle)?;
    project::compare(
        native.get("currency_owner_encoding").ok_or("encoding")?,
        &json!("bytes-v1"),
    )?;
    project::compare(
        native.get("before").ok_or("before")?,
        native.get("after").ok_or("after")?,
    )?;
    let control: Value =
        serde_json::from_slice(&std::fs::read(directory.join("native/control.json"))?)?;
    for field in ["random", "interactive_random"] {
        let original = control
            .get("before")
            .and_then(|v| v.get(field))
            .ok_or("control RNG")?;
        for phase in ["prepared", "after"] {
            project::compare(
                original,
                control
                    .get(phase)
                    .and_then(|v| v.get(field))
                    .ok_or("phase RNG")?,
            )?;
        }
        project::compare(
            original,
            native
                .get("before")
                .and_then(|v| v.get(field))
                .ok_or("currency RNG")?,
        )?;
    }
    let mut initial = CurrencyOwners::default();
    let custom = initial.entries.get_mut(31).ok_or("custom")?;
    custom.separator = ".".into();
    custom.suffix = b" credits".to_vec();
    project::compare(
        native.get("initial").ok_or("initial")?,
        &json!({"owners":initial.entries,"strings":{"selected":selected,"entries":[]}}),
    )?;
    let rust = expected(operations, selected)?;
    compare(native.get("results").ok_or("results")?, &rust, directory)
}
