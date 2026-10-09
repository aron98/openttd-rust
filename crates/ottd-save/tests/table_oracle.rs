//! Independent native-memory comparison for the recursive wire layer.
#![cfg(test)]
use ottd_save::{
    ChunkKind, Savegame, TableChunk, TableRecord, TableSchema, TableTailPolicy, WireValue,
};
use serde_json::Value;

fn record_json(record: &TableRecord, schema: &TableSchema) -> Value {
    Value::Object(
        schema
            .fields()
            .iter()
            .zip(record.values())
            .map(|(field, value)| {
                let json = match value {
                    WireValue::Signed(value) => Value::from(*value),
                    WireValue::Unsigned(value) => Value::from(*value),
                    WireValue::Bytes(bytes) => serde_json::to_value(bytes).unwrap(),
                    WireValue::Array(values) => Value::Array(
                        values
                            .iter()
                            .map(|value| match value {
                                WireValue::Signed(value) => Value::from(*value),
                                WireValue::Unsigned(value) => Value::from(*value),
                                WireValue::Bytes(_)
                                | WireValue::Array(_)
                                | WireValue::Structs(_) => panic!("nonprimitive array value"),
                            })
                            .collect(),
                    ),
                    WireValue::Structs(records) => Value::Array(
                        records
                            .iter()
                            .map(|record| record_json(record, field.child().unwrap()))
                            .collect(),
                    ),
                };
                (field.name().to_owned(), json)
            })
            .collect(),
    )
}

#[test]
#[ignore = "requires OTTD_WORLD_SAVE and OTTD_WORLD_JSON from the native world driver"]
fn matches_native_memory_and_preserves_populated_table_bytes() {
    // Given an independently captured native runtime tree and its matching save.
    let bytes = std::fs::read(std::env::var("OTTD_WORLD_SAVE").unwrap()).unwrap();
    let expected: Value =
        serde_json::from_slice(&std::fs::read(std::env::var("OTTD_WORLD_JSON").unwrap()).unwrap())
            .unwrap();
    let save = Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES).unwrap();
    let mut compared = 0;
    // When native tables are independently decoded by Rust.
    for chunk in save
        .chunks()
        .iter()
        .filter(|chunk| matches!(chunk.kind(), ChunkKind::Table | ChunkKind::SparseTable))
    {
        let script = matches!(&chunk.id(), b"AIPL" | b"GSDT");
        let policy = if script {
            TableTailPolicy::PreserveScriptData
        } else {
            TableTailPolicy::Reject
        };
        let table = TableChunk::decode(chunk, policy).unwrap();
        // Then every table byte survives; all nonscript table fields match native memory.
        assert_eq!(
            table.encode().unwrap().body(),
            chunk.body(),
            "chunk {:?}",
            chunk.id()
        );
        if script {
            continue;
        }
        let id = String::from_utf8(chunk.id().to_vec()).unwrap();
        let actual = Value::Object(
            table
                .records()
                .iter()
                .map(|(id, record)| (id.to_string(), record_json(record, table.schema())))
                .collect(),
        );
        assert_eq!(
            Some(&actual),
            expected
                .get("chunks")
                .and_then(|chunks| chunks.get(&id))
                .and_then(|chunk| chunk.get("records")),
            "chunk {id}"
        );
        compared += 1;
    }
    assert!(
        compared > 40,
        "populated oracle must cover the full table corpus"
    );
}
