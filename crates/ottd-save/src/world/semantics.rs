use super::{
    TableChunk, TableRecord, TableSchema, WireValue, WorldError, invalid, name, references,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    savegame_version: u16,
    native_commit: String,
    chunks: BTreeMap<String, Vec<Field>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Field {
    name: String,
    command: u8,
    file_type: u8,
    memory_type: u16,
    length: usize,
    reference_kind: Option<u8>,
    #[serde(default)]
    children: Vec<Self>,
}
pub(super) fn validate(tables: &BTreeMap<[u8; 4], TableChunk>) -> Result<(), WorldError> {
    let catalog: Catalog = serde_json::from_str(include_str!("native-schema-v362.json"))?;
    if catalog.schema_version != 1
        || catalog.savegame_version != 362
        || catalog.native_commit != "14ec60f248547d4d062a1160f0fc26d742319888"
    {
        return Err(invalid("native schema", "incorrect source pin"));
    }
    for (id, table) in tables {
        let fields = catalog
            .chunks
            .get(&name(*id))
            .ok_or_else(|| invalid("native schema", "missing chunk"))?;
        for (index, record) in table.records() {
            visit(
                tables,
                fields,
                table.schema(),
                record,
                &format!("{}[{index}]", name(*id)),
            )?;
        }
    }
    Ok(())
}
fn visit(
    tables: &BTreeMap<[u8; 4], TableChunk>,
    fields: &[Field],
    schema: &TableSchema,
    record: &TableRecord,
    path: &str,
) -> Result<(), WorldError> {
    if fields.len() != schema.fields().len() {
        return Err(invalid(path, "native descriptor count differs"));
    }
    for ((field, wire), value) in fields.iter().zip(schema.fields()).zip(record.values()) {
        let path = format!("{path}/{}", field.name);
        if field.name != wire.name() {
            return Err(invalid(&path, "native field name differs"));
        }
        if field.command == 0
            && field.memory_type == 0
            && !matches!(value, WireValue::Signed(0 | 1) | WireValue::Unsigned(0 | 1))
        {
            return Err(invalid(&path, "invalid native boolean"));
        }
        if field.command == 5
            && !matches!(value,WireValue::Array(items) if items.len()==field.length)
        {
            return Err(invalid(&path, "invalid native fixed array length"));
        }
        if let Some(kind) = field.reference_kind {
            let pool = match kind {
                1 | 4 => *b"VEHS",
                2 => *b"STNN",
                3 => *b"CITY",
                5 => *b"ROAD",
                6 => *b"ERNW",
                7 => *b"CAPA",
                8 => *b"ORDL",
                9 => *b"PSAC",
                10 => *b"LGRP",
                11 => *b"LGRJ",
                _ => return Err(invalid(&path, "unknown native reference kind")),
            };
            match value {
                WireValue::Unsigned(raw) => reference(tables, pool, *raw, &path, true)?,
                WireValue::Array(items) => {
                    for item in items {
                        match item {
                            WireValue::Unsigned(raw) => {
                                reference(tables, pool, *raw, &path, false)?;
                            }
                            _ => return Err(invalid(&path, "invalid reference element")),
                        }
                    }
                }
                _ => return Err(invalid(&path, "invalid reference shape")),
            }
        }
        if let WireValue::Structs(records) = value {
            if field.command == 2 && records.len() > 1 {
                return Err(invalid(&path, "too many optional struct records"));
            }
            let child = wire
                .child()
                .ok_or_else(|| invalid(&path, "missing child schema"))?;
            for (index, record) in records.iter().enumerate() {
                visit(
                    tables,
                    &field.children,
                    child,
                    record,
                    &format!("{path}[{index}]"),
                )?;
            }
        }
        if field.reference_kind.is_none() && field.file_type > 10 {
            return Err(invalid(&path, "unsupported native file type"));
        }
    }
    Ok(())
}
fn reference(
    tables: &BTreeMap<[u8; 4], TableChunk>,
    pool: [u8; 4],
    raw: u64,
    path: &str,
    nullable: bool,
) -> Result<(), WorldError> {
    if raw == 0 {
        return if nullable {
            Ok(())
        } else {
            Err(invalid(path, "null list reference"))
        };
    }
    let id = u32::try_from(
        raw.checked_sub(1)
            .ok_or_else(|| invalid(path, "invalid reference"))?,
    )
    .map_err(|_| invalid(path, "reference overflow"))?;
    references::require(tables, pool, id, path)?;
    if pool == *b"STNN" {
        let table = tables
            .get(&pool)
            .ok_or_else(|| invalid(path, "missing station pool"))?;
        let record = table
            .records()
            .get(&id)
            .ok_or_else(|| invalid(path, "missing station"))?;
        let station = super::Row {
            schema: table.schema(),
            record,
        };
        if station.unsigned("facilities")? & 0x40 != 0 {
            return Err(invalid(path, "station reference points to waypoint"));
        }
    }
    Ok(())
}
