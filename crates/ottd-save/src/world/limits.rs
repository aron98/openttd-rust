use super::{TableChunk, TableRecord, TableSchema, WireValue, WorldError, invalid, name, rows};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    savegame_version: u16,
    native_commit: String,
    singletons: BTreeMap<String, String>,
    record_limits: BTreeMap<String, RecordLimit>,
    list_limits: BTreeMap<String, BTreeMap<String, ListLimit>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordLimit {
    exclusive: u32,
    #[serde(rename = "source")]
    _source: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListLimit {
    minimum: usize,
    maximum: usize,
    #[serde(rename = "source")]
    _source: String,
}

pub(super) fn validate(tables: &BTreeMap<[u8; 4], TableChunk>) -> Result<(), WorldError> {
    let catalog: Catalog = serde_json::from_str(include_str!("handler-limits-v362.json"))?;
    if catalog.savegame_version != 362
        || catalog.native_commit != "14ec60f248547d4d062a1160f0fc26d742319888"
    {
        return Err(invalid("handler limits", "incorrect source pin"));
    }
    for (id, table) in tables {
        let name = name(*id);
        if catalog.singletons.contains_key(&name) && table.records().len() > 1 {
            return Err(invalid(&name, "multiple records in singleton chunk"));
        }
        if let Some(limit) = catalog.record_limits.get(&name) {
            if table.records().keys().any(|id| *id >= limit.exclusive) {
                return Err(invalid(&name, "record index exceeds native handler limit"));
            }
        }
        if let Some(limits) = catalog.list_limits.get(&name) {
            for (index, row) in table.records() {
                visit(limits, table.schema(), row, "", &format!("{name}[{index}]"))?;
            }
        }
    }
    metadata(tables)
}
fn visit(
    limits: &BTreeMap<String, ListLimit>,
    schema: &TableSchema,
    row: &TableRecord,
    prefix: &str,
    object: &str,
) -> Result<(), WorldError> {
    for (field, value) in schema.fields().iter().zip(row.values()) {
        let WireValue::Structs(records) = value else {
            continue;
        };
        let path = if prefix.is_empty() {
            field.name().to_owned()
        } else {
            format!("{prefix}/{}", field.name())
        };
        if let Some(limit) = limits.get(&path) {
            if !(limit.minimum..=limit.maximum).contains(&records.len()) {
                return Err(invalid(
                    &format!("{object}/{path}"),
                    "list exceeds native handler storage bounds",
                ));
            }
        }
        let child = field
            .child()
            .ok_or_else(|| invalid(&path, "missing child schema"))?;
        for record in records {
            visit(limits, child, record, &path, object)?;
        }
    }
    Ok(())
}
fn metadata(tables: &BTreeMap<[u8; 4], TableChunk>) -> Result<(), WorldError> {
    for (_, row) in rows(tables, *b"EIDS")? {
        if row.unsigned("type")? >= 4 {
            return Err(invalid("EIDS/type", "invalid engine mapping vehicle type"));
        }
    }
    for (_, row) in rows(tables, *b"CITY")? {
        let kind = row.unsigned("townnametype")?;
        if row.unsigned("townnamegrfid")? == 0
            && !(0x20c0..0x20d5).contains(&kind)
            && kind >> 11 != 15
        {
            return Err(invalid(
                "CITY/townnametype",
                "invalid built-in town name generator",
            ));
        }
    }
    for (_, row) in rows(tables, *b"GLOG")? {
        for action in row.child("action")? {
            let kind = usize::try_from(action.unsigned("ct")?)
                .map_err(|_| invalid("GLOG/ct", "invalid change type"))?;
            let variants = [
                "mode",
                "revision",
                "oldver",
                "setting",
                "grfadd",
                "grfrem",
                "grfcompat",
                "grfparam",
                "grfmove",
                "grfbug",
                "emergency",
            ];
            if kind >= variants.len() {
                return Err(invalid("GLOG/ct", "invalid change type"));
            }
            for (index, name) in variants.iter().enumerate() {
                if action.child(name)?.len() != usize::from(index == kind) {
                    return Err(invalid(
                        "GLOG/action",
                        "inconsistent gamelog change variant",
                    ));
                }
            }
        }
    }
    Ok(())
}
