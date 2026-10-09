use super::{
    Savegame, TableRecord, TableSchema, WireValue, World, WorldError, invalid, name, scripts,
};
use crate::Compression;
use std::collections::BTreeMap;

impl World {
    /// Serialize authoritative state into a native save container.
    /// # Errors
    /// Propagates table/container encoding failures.
    pub fn to_savegame(&self) -> Result<Savegame, WorldError> {
        self.build_save(&self.planes)
    }

    /// Encode a native-compatible save using the requested compression.
    /// # Errors
    /// Propagates table/container/compression failures.
    pub fn encode(&self, compression: Compression) -> Result<Vec<u8>, WorldError> {
        Ok(self.to_savegame()?.encode(compression)?)
    }

    pub(super) fn build_save(
        &self,
        planes: &BTreeMap<[u8; 4], Vec<u8>>,
    ) -> Result<Savegame, WorldError> {
        let mut bytes = b"OTTN".to_vec();
        bytes.extend_from_slice(&self.version_bytes);
        for id in &self.order {
            if let Some(table) = self.tables.get(id) {
                table.encode()?.write_to(&mut bytes)?;
            } else {
                let plane = planes
                    .get(id)
                    .ok_or_else(|| invalid(&name(*id), "missing plane"))?;
                bytes.extend_from_slice(id);
                let length =
                    u32::try_from(plane.len()).map_err(|_| invalid("map", "plane too large"))?;
                let [high, a, b, c] = length.to_be_bytes();
                bytes.extend_from_slice(&[high << 4, a, b, c]);
                bytes.extend_from_slice(plane);
            }
        }
        bytes.extend_from_slice(&[0; 4]);
        Ok(Savegame::decode(&bytes, crate::DEFAULT_MAX_BYTES)?)
    }

    /// Canonical complete saved-state tree used by the independent native oracle.
    /// # Errors
    /// Rejects malformed script-tail data encountered during export.
    pub fn saved_json(&self) -> Result<serde_json::Value, WorldError> {
        let mut chunks = serde_json::Map::new();
        for (id, table) in &self.tables {
            let mut rows = serde_json::Map::new();
            for (index, record) in table.records() {
                let mut fields = record_json(table.schema(), record);
                scripts::export(*id, record, &mut fields)?;
                rows.insert(index.to_string(), serde_json::Value::Object(fields));
            }
            chunks.insert(name(*id), serde_json::json!({"records":rows}));
        }
        for (id, bytes) in &self.planes {
            chunks.insert(name(*id), serde_json::json!({"bytes":bytes}));
        }
        Ok(serde_json::json!({"schema_version":1,"savegame_version":362,"chunks":chunks}))
    }
}

fn record_json(
    schema: &TableSchema,
    row: &TableRecord,
) -> serde_json::Map<String, serde_json::Value> {
    schema
        .fields()
        .iter()
        .zip(row.values())
        .map(|(field, value)| {
            let json = match value {
                WireValue::Signed(n) => serde_json::json!(n),
                WireValue::Unsigned(n) => serde_json::json!(n),
                WireValue::Bytes(bytes) => serde_json::json!(bytes),
                WireValue::Array(values) => {
                    serde_json::Value::Array(values.iter().map(atomic_json).collect())
                }
                WireValue::Structs(records) => {
                    serde_json::Value::Array(field.child().map_or_else(Vec::new, |child| {
                        records
                            .iter()
                            .map(|row| serde_json::Value::Object(record_json(child, row)))
                            .collect()
                    }))
                }
            };
            (field.name().to_owned(), json)
        })
        .collect()
}
fn atomic_json(value: &WireValue) -> serde_json::Value {
    match value {
        WireValue::Signed(n) => serde_json::json!(n),
        WireValue::Unsigned(n) => serde_json::json!(n),
        WireValue::Bytes(bytes) => serde_json::json!(bytes),
        WireValue::Array(values) => {
            serde_json::Value::Array(values.iter().map(atomic_json).collect())
        }
        WireValue::Structs(_) => serde_json::Value::Null,
    }
}
