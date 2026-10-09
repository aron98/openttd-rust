//! Complete v362 saved state and content-independent structural restoration.
mod cargo;
mod derived;
mod edit;
mod export;
mod orders;
mod ownership;
mod references;
mod scripts;
mod semantics;

use crate::{
    ChunkKind, MapState, Savegame, TableChunk, TableError, TableRecord, TableSchema,
    TableTailPolicy, TileState, WireValue, WorldSnapshot,
};
pub use derived::{
    CargoAggregate, CargoPayment, DerivedState, GroupChildren, OrderListState, Owner, Pool,
    RoadStopChain, StorageOwner, VehicleLinks,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A field name or a zero-based list position in an edit path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathElement {
    /// Exact saved field name; embedded dots are not separators.
    Field(String),
    /// Nested structure or primitive array position.
    Index(usize),
}

/// One mutation staged in a complete saved-world transaction.
#[derive(Debug, Clone)]
pub enum WorldEdit {
    /// Replace an existing field while preserving object identity.
    Field {
        /// Native chunk name.
        chunk: [u8; 4],
        /// Pool record index.
        record: u32,
        /// Exact field/list path.
        path: Vec<PathElement>,
        /// Replacement wire value.
        value: WireValue,
    },
    /// Replace one tile's saved representation.
    Tile {
        /// Linear tile index.
        index: u32,
        /// Complete tile state.
        value: TileState,
    },
}

/// Invalid or unsupported saved-world state.
#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    /// Structural or semantic failure at a named object/field.
    #[error("invalid world at {path}: {reason}")]
    Invalid {
        /// Chunk, object and field location.
        path: String,
        /// Failure category.
        reason: &'static str,
    },
    /// Unsupported typed save version.
    #[error("world loading requires version 362, got {0}")]
    Version(u16),
    /// Wire table failure.
    #[error(transparent)]
    Table(#[from] TableError),
    /// Container failure.
    #[error(transparent)]
    Container(#[from] crate::Error),
    /// Existing typed map, clock or settings validation failure.
    #[error(transparent)]
    Snapshot(#[from] crate::SnapshotError),
    /// Bundled schema manifest could not be read.
    #[error("invalid bundled schema: {0}")]
    Schema(#[from] serde_json::Error),
}

/// Authoritative saved tables, raw map planes and restored structural indexes.
/// Content-dependent gameplay caches are deliberately not synthesized.
#[derive(Debug, Clone)]
pub struct World {
    tables: BTreeMap<[u8; 4], TableChunk>,
    planes: BTreeMap<[u8; 4], Vec<u8>>,
    order: Vec<[u8; 4]>,
    version_bytes: [u8; 4],
    snapshot: WorldSnapshot,
    derived: DerivedState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    savegame_version: u16,
    native_commit: String,
    schemas: BTreeMap<String, TableSchema>,
}

impl World {
    /// Decode every saved domain and restore the supported structural state.
    ///
    /// # Errors
    /// Rejects unknown/missing chunks, changed schemas, bad references, ownership
    /// violations, malformed script data and cyclic linked structures.
    pub fn decode(save: &Savegame) -> Result<Self, WorldError> {
        if save.version() != crate::SAVEGAME_VERSION {
            return Err(WorldError::Version(save.version()));
        }
        let manifest: Manifest = serde_json::from_str(include_str!("world/schema-v362.json"))?;
        if manifest.savegame_version != 362
            || manifest.native_commit != "14ec60f248547d4d062a1160f0fc26d742319888"
        {
            return Err(invalid("schema", "incorrect source pin"));
        }
        let mut tables = BTreeMap::new();
        let mut planes = BTreeMap::new();
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        for chunk in save.chunks() {
            let id = chunk.id();
            let name = name(id);
            if !seen.insert(id) {
                return Err(invalid(&name, "duplicate chunk"));
            }
            order.push(id);
            match &id {
                b"MAPT" | b"MAPH" | b"MAPO" | b"MAP2" | b"M3LO" | b"M3HI" | b"MAP5" | b"MAPE"
                | b"MAP7" | b"MAP8" => {
                    if chunk.kind() != ChunkKind::Riff {
                        return Err(invalid(&name, "map plane is not RIFF"));
                    }
                    planes.insert(id, chunk.body().to_vec());
                }
                _ => {
                    let expected = manifest
                        .schemas
                        .get(&name)
                        .ok_or_else(|| invalid(&name, "unknown or obsolete v362 chunk"))?;
                    let policy = match &id {
                        b"AIPL" | b"GSDT" => TableTailPolicy::PreserveScriptData,
                        _ => TableTailPolicy::Reject,
                    };
                    let table = TableChunk::decode(chunk, policy)?;
                    if table.schema() != expected {
                        return Err(invalid(
                            &name,
                            "schema differs from pinned native v362 descriptors",
                        ));
                    }
                    tables.insert(id, table);
                }
            }
        }
        for required in manifest.schemas.keys() {
            let id: [u8; 4] = required
                .as_bytes()
                .try_into()
                .map_err(|_| invalid(required, "invalid manifest chunk name"))?;
            if !tables.contains_key(&id) {
                return Err(invalid(required, "missing required chunk"));
            }
        }
        let snapshot = save.snapshot()?;
        semantics::validate(&tables)?;
        references::validate(&tables, snapshot.map())?;
        scripts::validate(&tables)?;
        let derived = derived::restore(&tables)?;
        Ok(Self {
            tables,
            planes,
            order,
            version_bytes: save.version_bytes,
            snapshot,
            derived,
        })
    }

    /// Complete saved table state keyed by four-byte native chunk name.
    pub const fn tables(&self) -> &BTreeMap<[u8; 4], TableChunk> {
        &self.tables
    }
    /// Validated dimensions and every saved tile bit.
    pub const fn map(&self) -> &MapState {
        self.snapshot.map()
    }
    /// Content-independent reconstructed indexes and aggregates.
    pub const fn derived(&self) -> &DerivedState {
        &self.derived
    }
}

fn name(id: [u8; 4]) -> String {
    String::from_utf8_lossy(&id).into_owned()
}
fn invalid(path: &str, reason: &'static str) -> WorldError {
    WorldError::Invalid {
        path: path.to_owned(),
        reason,
    }
}

#[derive(Clone, Copy)]
struct Row<'a> {
    schema: &'a TableSchema,
    record: &'a TableRecord,
}
impl<'a> Row<'a> {
    fn value(self, name: &str) -> Result<&'a WireValue, WorldError> {
        self.schema
            .fields()
            .iter()
            .zip(self.record.values())
            .find_map(|(f, v)| (f.name() == name).then_some(v))
            .ok_or_else(|| invalid(name, "missing semantic field"))
    }
    fn unsigned(self, name: &str) -> Result<u64, WorldError> {
        match self.value(name)? {
            WireValue::Unsigned(n) => Ok(*n),
            _ => Err(invalid(name, "expected unsigned field")),
        }
    }
    fn signed(self, name: &str) -> Result<i64, WorldError> {
        match self.value(name)? {
            WireValue::Signed(n) => Ok(*n),
            _ => Err(invalid(name, "expected signed field")),
        }
    }
    fn child(self, name: &str) -> Result<Vec<Self>, WorldError> {
        let field = self
            .schema
            .fields()
            .iter()
            .find(|f| f.name() == name)
            .ok_or_else(|| invalid(name, "missing struct field"))?;
        let schema = field
            .child()
            .ok_or_else(|| invalid(name, "expected child schema"))?;
        match self.value(name)? {
            WireValue::Structs(rows) => {
                Ok(rows.iter().map(|record| Self { schema, record }).collect())
            }
            _ => Err(invalid(name, "expected struct list")),
        }
    }
    fn single(self, name: &str) -> Result<Self, WorldError> {
        let rows = self.child(name)?;
        if rows.len() != 1 {
            return Err(invalid(name, "expected exactly one structure"));
        }
        rows.first()
            .copied()
            .ok_or_else(|| invalid(name, "missing structure"))
    }
    fn reference(self, name: &str) -> Result<Option<u32>, WorldError> {
        let raw =
            u32::try_from(self.unsigned(name)?).map_err(|_| invalid(name, "reference overflow"))?;
        Ok(raw.checked_sub(1))
    }
    fn references(self, name: &str) -> Result<Vec<u32>, WorldError> {
        match self.value(name)? {
            WireValue::Array(values) => values
                .iter()
                .map(|v| match v {
                    WireValue::Unsigned(n) => u32::try_from(*n)
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .ok_or_else(|| invalid(name, "null/overflow list reference")),
                    _ => Err(invalid(name, "expected reference list")),
                })
                .collect(),
            _ => Err(invalid(name, "expected reference list")),
        }
    }
}
fn rows(
    tables: &BTreeMap<[u8; 4], TableChunk>,
    id: [u8; 4],
) -> Result<impl Iterator<Item = (u32, Row<'_>)>, WorldError> {
    let table = tables
        .get(&id)
        .ok_or_else(|| invalid(&name(id), "missing table"))?;
    Ok(table.records().iter().map(move |(id, record)| {
        (
            *id,
            Row {
                schema: table.schema(),
                record,
            },
        )
    }))
}
