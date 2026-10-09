//! Regenerate the source-pinned world schema asset from native save headers.
#![cfg(test)]
use ottd_save::{ChunkKind, Savegame, TableChunk, TableTailPolicy};
use std::collections::BTreeMap;

#[test]
#[ignore = "explicit schema generation; WORLD_SCHEMA_OUTPUT names the destination"]
fn generate_schema_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let save = Savegame::decode(
        include_bytes!("../../../fixtures/generated-v362.sav"),
        4 * 1024 * 1024,
    )?;
    let mut schemas = BTreeMap::new();
    for chunk in save.chunks() {
        if matches!(chunk.kind(), ChunkKind::Table | ChunkKind::SparseTable) {
            let policy = if matches!(&chunk.id(), b"AIPL" | b"GSDT") {
                TableTailPolicy::PreserveScriptData
            } else {
                TableTailPolicy::Reject
            };
            let table = TableChunk::decode(chunk, policy)?;
            schemas.insert(
                String::from_utf8_lossy(&chunk.id()).into_owned(),
                table.schema().clone(),
            );
        }
    }
    let manifest = serde_json::json!({"savegame_version":362,"native_commit":"14ec60f248547d4d062a1160f0fc26d742319888","schemas":schemas});
    let path = std::env::var("WORLD_SCHEMA_OUTPUT")?;
    std::fs::write(path, serde_json::to_string_pretty(&manifest)? + "\n")?;
    Ok(())
}

#[test]
#[ignore = "explicit generation from fresh WORLD_NATIVE_SCHEMA_INPUT into WORLD_NATIVE_SCHEMA_OUTPUT"]
fn generate_native_descriptor_catalog() -> Result<(), Box<dyn std::error::Error>> {
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var("WORLD_NATIVE_SCHEMA_INPUT")?)?)?;
    let root = catalog
        .as_object_mut()
        .ok_or("native schema is not an object")?;
    if root.get("schema_version") != Some(&serde_json::json!(1))
        || root.get("savegame_version") != Some(&serde_json::json!(362))
    {
        return Err("native schema version mismatch".into());
    }
    root.insert(
        "native_commit".to_owned(),
        serde_json::json!("14ec60f248547d4d062a1160f0fc26d742319888"),
    );
    std::fs::write(
        std::env::var("WORLD_NATIVE_SCHEMA_OUTPUT")?,
        serde_json::to_string_pretty(&catalog)? + "\n",
    )?;
    Ok(())
}
