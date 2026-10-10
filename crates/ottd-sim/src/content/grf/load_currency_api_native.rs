use super::{
    language_pack::{BuiltinPack, Pack},
    load_currency::{CurrencyOwner, CurrencyState},
    load_currency_native::{Result, compare, invoke, setup},
    load_string_mapping::map_string,
    load_strings::{Definition, StringKey, StringTable},
    text::Budget,
    text_mapped::TextContext,
    text_translate::{Input, translate_in},
};
use serde_json::{Value, json};

#[derive(serde::Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum Operation {
    ResetCurrencies {
        preserve_custom: bool,
    },
    CustomFixture {
        owner: CurrencyOwner,
    },
    ResetStrings,
    Define {
        grfid: u32,
        local_id: u16,
        language: u8,
        raw: Vec<u8>,
    },
    Queue {
        grfid: u32,
        first: u16,
        count: u8,
        reserve: bool,
        raw: Vec<u8>,
    },
    Finalize,
    Map {
        grfid: u32,
        queries: Vec<u16>,
    },
    Read {
        id: u32,
    },
}

fn expected(operations: &[Operation], bytes: &[u8]) -> Result<Value> {
    let pack = Pack::header(bytes)?;
    let builtins = BuiltinPack::new(bytes)?;
    let mut currency = CurrencyState::default();
    let mut strings = StringTable::default();
    let mut budget = Budget::new(super::ScanLimits::default());
    let mut rows = Vec::new();
    for operation in operations {
        let serialized = serde_json::to_value(operation)?;
        let mut row = serde_json::Map::new();
        row.insert(
            "operation".into(),
            serialized.get("operation").ok_or("operation tag")?.clone(),
        );
        match operation {
            Operation::ResetCurrencies { preserve_custom } => {
                currency.owners =
                    super::load_currency::CurrencyOwners::reset(if *preserve_custom {
                        currency.owners.entries.get(31)
                    } else {
                        None
                    });
            }
            Operation::CustomFixture { owner } => {
                *currency.owners.entries.get_mut(31).ok_or("custom owner")? = owner.clone();
                row.insert("declared_custom_owner".into(), json!(owner));
            }
            Operation::ResetStrings => strings = StringTable::default(),
            Operation::Define {
                grfid,
                local_id,
                language,
                raw,
            } => {
                let definition = Definition {
                    key: StringKey {
                        grfid: *grfid,
                        local_id: u32::from(*local_id),
                    },
                    language: *language,
                    new_scheme: true,
                    default_id: 2,
                };
                let id = define_string(&mut strings, definition, raw, &pack, &mut budget)?;
                row.insert("id".into(), json!(id));
            }
            Operation::Queue {
                grfid,
                first,
                count,
                reserve,
                raw,
            } => {
                let (bounds, remaining) =
                    apply_queue(&mut currency, *grfid, *first, *count, *reserve, raw);
                if bounds {
                    row.insert("read_bounds".into(), json!(true));
                } else {
                    row.insert("result".into(), json!(0));
                }
                row.insert("remaining".into(), json!(remaining));
            }
            Operation::Finalize => currency.finalize(&strings),
            Operation::Map { grfid, queries } => {
                row.insert(
                    "ids".into(),
                    json!(
                        queries
                            .iter()
                            .map(|id| map_string(&strings, *grfid, *id))
                            .collect::<Vec<_>>()
                    ),
                );
            }
            Operation::Read { id } => {
                row.insert(
                    "bytes".into(),
                    json!(strings.resolve(*id, pack.language, |id| builtins.lookup(id))?),
                );
            }
        }
        row.insert("owners".into(), json!(currency.owners.entries));
        row.insert(
            "pending".into(),
            json!(
                currency
                    .pending
                    .iter()
                    .map(|entry| json!({"grfid":entry.grfid,"source":entry.source}))
                    .collect::<Vec<_>>()
            ),
        );
        row.insert("strings".into(), json!({"selected":pack.language,"entries":strings.entries.iter().map(|entry|json!([entry.key.grfid,entry.key.local_id,entry.default_id,entry.translations])).collect::<Vec<_>>()}));
        rows.push(row);
    }
    Ok(json!(rows))
}

fn define_string(
    strings: &mut StringTable,
    definition: Definition,
    raw: &[u8],
    pack: &Pack,
    budget: &mut Budget,
) -> Result<u32> {
    Ok(strings.define(definition, |table, _| {
        translate_in(
            Input {
                raw,
                newlines: false,
                offset: 0,
                context: TextContext {
                    map: None,
                    genders: pack.gender_count,
                    cases: pack.case_count,
                },
            },
            budget,
            |local| Some(table.inline_id(definition.key.grfid, local)),
        )
    })?)
}

fn apply_queue(
    currency: &mut CurrencyState,
    grfid: u32,
    first: u16,
    count: u8,
    reserve: bool,
    raw: &[u8],
) -> (bool, usize) {
    let mut reader = super::records::Reader { bytes: raw, pos: 0 };
    for index in u32::from(first)..u32::from(first).saturating_add(u32::from(count)) {
        let Ok(source) = reader.word() else {
            return (true, reader.remaining());
        };
        if !reserve {
            currency.queue(grfid, index, source);
        }
    }
    (false, reader.remaining())
}

fn queue(first: u16, count: u8, reserve: bool, ids: &[u16]) -> Operation {
    Operation::Queue {
        grfid: 7,
        first,
        count,
        reserve,
        raw: ids.iter().flat_map(|id| id.to_le_bytes()).collect(),
    }
}

fn cases() -> Vec<(&'static str, Vec<Operation>)> {
    let define = || Operation::Define {
        grfid: 7,
        local_id: 0xd800,
        language: 0x7f,
        raw: b"currency".to_vec(),
    };
    let mut all_indices = (0..=255_u16)
        .map(|index| queue(index, 1, false, &[0xd800]))
        .collect::<Vec<_>>();
    all_indices.extend([define(), Operation::Finalize]);
    let owner = CurrencyOwner {
        rate: 91,
        separator: "|".into(),
        to_euro: 2040,
        prefix: "before".into(),
        suffix: "after".into(),
        code: "ZZZ".into(),
        symbol_pos: 2,
        name: 1,
    };
    vec![
        ("all-owner-indices", all_indices),
        (
            "all-string-mappings",
            vec![
                define(),
                Operation::Define {
                    grfid: 7,
                    local_id: 0xd000,
                    language: 0x7f,
                    raw: b"alias".to_vec(),
                },
                Operation::Map {
                    grfid: 7,
                    queries: (0..=u16::MAX).collect(),
                },
                Operation::Map {
                    grfid: 8,
                    queries: vec![0xd000, 0xd400, 0xd800, 0xffff],
                },
            ],
        ),
        (
            "ordered-writes",
            vec![
                queue(0, 1, false, &[0xd800]),
                queue(0, 1, false, &[0]),
                define(),
                Operation::Finalize,
                Operation::Read { id: 1 },
                queue(256, 1, false, &[0xd800]),
                Operation::Finalize,
                Operation::Read { id: 0x20000 },
            ],
        ),
        (
            "reserve-bounds",
            vec![
                queue(0, 2, true, &[0xd800, 0]),
                queue(65535, 2, false, &[0xd800, 0xd800]),
                queue(46, 1, false, &[]),
                queue(0, 2, false, &[0xd800]),
                Operation::Finalize,
                queue(1, 0, false, &[]),
            ],
        ),
        (
            "custom-reset",
            vec![
                Operation::CustomFixture { owner },
                Operation::ResetCurrencies {
                    preserve_custom: true,
                },
                Operation::ResetCurrencies {
                    preserve_custom: false,
                },
                define(),
                queue(31, 1, false, &[0xd800]),
                Operation::Finalize,
                Operation::ResetStrings,
                Operation::ResetCurrencies {
                    preserve_custom: true,
                },
                Operation::ResetCurrencies {
                    preserve_custom: false,
                },
            ],
        ),
    ]
}

#[test]
#[ignore = "requires original currency API observer and fresh absolute artifacts"]
fn original_currency_api_matrix() -> Result {
    let (root, directory, oracle, pack) = setup()?;
    let selected = Pack::header(&pack)?.language;
    let mut summary = Vec::new();
    for (name, operations) in cases() {
        let case = directory.join(name);
        std::fs::create_dir(&case)?;
        let packs = case.join("pack");
        std::fs::create_dir(&packs)?;
        std::fs::write(packs.join("input.lng"), &pack)?;
        let manifest = json!({"files":[],"networking":false,"language":{"pack_directories":[packs],"selected":selected,"queries":[]},"currency_api":operations});
        let native = invoke(&root, &case, &manifest, &oracle)?;
        let rust = expected(&operations, &pack)?;
        let controls = compare(
            native.get("results").ok_or("native API results")?,
            &rust,
            &case,
        )?;
        summary.push(json!({"case":name,"operations":operations.len(),"controls":controls}));
    }
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(&summary)?,
    )?;
    println!("compared {} original currency API cases", summary.len());
    Ok(())
}
