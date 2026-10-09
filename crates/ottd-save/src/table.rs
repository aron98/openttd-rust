//! Editable recursive wire tables. These types do not resolve game references.
mod read;
mod write;

use crate::{Chunk, ChunkKind};
use std::collections::BTreeMap;

/// Resource limits shared by decoding and encoding a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableLimits {
    /// Maximum nested schema depth, including the root.
    pub max_depth: usize,
    /// Maximum total fields, values, records and array elements.
    pub max_elements: usize,
    /// Maximum wire bytes and separately accounted owned allocation bytes.
    pub max_bytes: usize,
}
impl Default for TableLimits {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_elements: 4_000_000,
            max_bytes: crate::DEFAULT_MAX_BYTES,
        }
    }
}

/// Explicit handling of data outside the table descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableTailPolicy {
    /// Every record must be entirely described by its schema.
    Reject,
    /// Preserve opaque trailing script data, only for AIPL and GSDT.
    /// This does not interpret or validate script state.
    PreserveScriptData,
}

/// Invalid table data, unsupported shapes, or resource limits.
#[derive(Debug, thiserror::Error)]
pub enum TableError {
    /// Invalid framing inside a table.
    #[error(transparent)]
    Container(#[from] crate::Error),
    /// A schema, record or value violates its wire contract.
    #[error("invalid table at {path}: {reason}")]
    Invalid {
        /// Record or field path within this table.
        path: String,
        /// Stable failure category.
        reason: &'static str,
    },
    /// A configured resource limit was exceeded.
    #[error("table exceeds {0} limit")]
    Limit(&'static str),
}

/// Ordered recursive field descriptors read from a native table header.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableSchema {
    fields: Vec<FieldSchema>,
}
impl TableSchema {
    /// Fields in their serialized order.
    pub fn fields(&self) -> &[FieldSchema] {
        &self.fields
    }
}

/// A wire descriptor; reference and enum semantics belong to the world model.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldSchema {
    name: String,
    wire_type: u8,
    child: Option<TableSchema>,
}
impl FieldSchema {
    /// Native field name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Exact native type byte, including the length flag.
    pub const fn wire_type(&self) -> u8 {
        self.wire_type
    }
    /// Schema shared by each record of a struct list.
    pub const fn child(&self) -> Option<&TableSchema> {
        self.child.as_ref()
    }
}

/// A value whose exact width and list element type are specified by its field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireValue {
    /// A signed wire integer.
    Signed(i64),
    /// An unsigned wire integer, including string IDs and unresolved references.
    Unsigned(u64),
    /// Native string bytes, preserved without UTF-8 normalization.
    Bytes(Vec<u8>),
    /// Ordered primitive values.
    Array(Vec<Self>),
    /// Count-prefixed records; optional structures have zero or one record.
    Structs(Vec<TableRecord>),
}

/// Values in schema order and an optional explicitly permitted script tail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRecord {
    values: Vec<WireValue>,
    tail: Vec<u8>,
}
impl TableRecord {
    /// Construct an ordinary wire record without a script tail.
    /// Schema, widths and relationships are checked by table encoding and world edits.
    pub const fn new(values: Vec<WireValue>) -> Self {
        Self {
            values,
            tail: Vec::new(),
        }
    }
    /// Values in the owning schema's field order.
    pub fn values(&self) -> &[WireValue] {
        &self.values
    }
    /// Edit existing values; encoding checks widths, shapes and resource limits.
    pub fn values_mut(&mut self) -> &mut [WireValue] {
        &mut self.values
    }
    /// Opaque script bytes, never accepted for ordinary chunks or nested records.
    pub fn tail(&self) -> &[u8] {
        &self.tail
    }
}

/// Editable table wire state, not a validated or restored runtime world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableChunk {
    id: [u8; 4],
    kind: ChunkKind,
    schema: TableSchema,
    records: BTreeMap<u32, TableRecord>,
    slots: u32,
    tail_policy: TableTailPolicy,
    limits: TableLimits,
}
impl TableChunk {
    /// Decode an ordinary or sparse table with default resource limits.
    ///
    /// # Errors
    /// Rejects malformed schemas, records, duplicate IDs and disallowed tails.
    pub fn decode(chunk: &Chunk, tail_policy: TableTailPolicy) -> Result<Self, TableError> {
        Self::decode_with_limits(chunk, tail_policy, TableLimits::default())
    }
    /// Decode with explicit recursive, element and allocation limits.
    ///
    /// # Errors
    /// Returns a framing, shape or resource-limit error.
    pub fn decode_with_limits(
        chunk: &Chunk,
        tail_policy: TableTailPolicy,
        limits: TableLimits,
    ) -> Result<Self, TableError> {
        read::decode(chunk, tail_policy, limits)
    }
    /// Rebuild the table, validating edited values against the original schema.
    ///
    /// # Errors
    /// Rejects invalid shapes, integer widths, tails, indices or exceeded limits.
    pub fn encode(&self) -> Result<Chunk, TableError> {
        write::encode(self)
    }

    pub(crate) fn validated_wire_len(&self) -> Result<usize, TableError> {
        write::validate(self)
    }

    pub(crate) fn normalize_slots(&mut self) {
        self.slots = match self.kind {
            ChunkKind::Table => self.slots.max(
                self.records
                    .last_key_value()
                    .map_or(0, |(id, _)| id.saturating_add(1)),
            ),
            ChunkKind::SparseTable => u32::try_from(self.records.len()).unwrap_or(u32::MAX),
            ChunkKind::Riff | ChunkKind::Array | ChunkKind::SparseArray => self.slots,
        };
    }
    /// Four-byte chunk identity.
    pub const fn id(&self) -> [u8; 4] {
        self.id
    }
    /// Original ordinary/sparse table encoding.
    pub const fn kind(&self) -> ChunkKind {
        self.kind
    }
    /// Original recursive schema, including field ordering.
    pub const fn schema(&self) -> &TableSchema {
        &self.schema
    }
    /// Present records keyed by exact native pool index.
    pub const fn records(&self) -> &BTreeMap<u32, TableRecord> {
        &self.records
    }
    /// Edit records. Encoding validates the resulting wire representation.
    pub const fn records_mut(&mut self) -> &mut BTreeMap<u32, TableRecord> {
        &mut self.records
    }
}

fn invalid(path: &str, reason: &'static str) -> TableError {
    TableError::Invalid {
        path: path.to_owned(),
        reason,
    }
}
fn allow_tail(id: [u8; 4], policy: TableTailPolicy) -> Result<bool, TableError> {
    match policy {
        TableTailPolicy::Reject => Ok(false),
        TableTailPolicy::PreserveScriptData => match &id {
            b"AIPL" | b"GSDT" => Ok(true),
            _ => Err(invalid("chunk", "script tail policy requires AIPL or GSDT")),
        },
    }
}

struct Budget {
    limits: TableLimits,
    elements: usize,
    bytes: usize,
}
impl Budget {
    const fn new(limits: TableLimits) -> Self {
        Self {
            limits,
            elements: 0,
            bytes: 0,
        }
    }
    const fn depth(&self, depth: usize) -> Result<(), TableError> {
        if depth > self.limits.max_depth {
            Err(TableError::Limit("depth"))
        } else {
            Ok(())
        }
    }
    fn charge(&mut self, elements: usize, bytes: usize) -> Result<(), TableError> {
        self.elements = self
            .elements
            .checked_add(elements)
            .ok_or(TableError::Limit("elements"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(TableError::Limit("allocation bytes"))?;
        if self.elements > self.limits.max_elements {
            return Err(TableError::Limit("elements"));
        }
        if self.bytes > self.limits.max_bytes {
            return Err(TableError::Limit("allocation bytes"));
        }
        Ok(())
    }
    fn items<T>(&mut self, count: usize) -> Result<(), TableError> {
        self.charge(
            count,
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(TableError::Limit("allocation bytes"))?,
        )
    }
}
