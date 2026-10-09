//! Shared schema-based access to authoritative saved fields.
#![expect(
    clippy::redundant_pub_crate,
    reason = "shared adapter is intentionally internal to commands and world ticks"
)]
#![expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "chunk keys are borrowed directly for table lookup and shared command callers"
)]
use ottd_save::{
    TableRecord, TableSchema, WireValue,
    world::{PathElement, World, WorldEdit},
};

/// A saved field required by the supported runtime has another shape.
#[derive(Debug, thiserror::Error)]
#[error("saved field {0} has an unsupported value or shape")]
pub struct WorldAccessError(pub String);

pub(super) fn field<'a>(
    world: &'a World,
    chunk: &[u8; 4],
    id: u32,
    name: &str,
) -> Result<&'a WireValue, WorldAccessError> {
    let table = world
        .tables()
        .get(chunk)
        .ok_or_else(|| WorldAccessError(String::from_utf8_lossy(chunk).into_owned()))?;
    let row = table
        .records()
        .get(&id)
        .ok_or_else(|| WorldAccessError(format!("{chunk:?}/{id}")))?;
    row_field(table.schema(), row, name)
}
pub(super) fn row_field<'a>(
    schema: &TableSchema,
    row: &'a TableRecord,
    name: &str,
) -> Result<&'a WireValue, WorldAccessError> {
    schema
        .fields()
        .iter()
        .zip(row.values())
        .find_map(|(f, v)| (f.name() == name).then_some(v))
        .ok_or_else(|| WorldAccessError(name.into()))
}
pub(super) fn unsigned(
    world: &World,
    chunk: &[u8; 4],
    id: u32,
    name: &str,
) -> Result<u64, WorldAccessError> {
    match field(world, chunk, id, name)? {
        WireValue::Unsigned(v) => Ok(*v),
        WireValue::Signed(v) => u64::try_from(*v).map_err(|_| WorldAccessError(name.into())),
        _ => Err(WorldAccessError(name.into())),
    }
}
pub(super) fn signed(
    world: &World,
    chunk: &[u8; 4],
    id: u32,
    name: &str,
) -> Result<i64, WorldAccessError> {
    match field(world, chunk, id, name)? {
        WireValue::Signed(v) => Ok(*v),
        _ => Err(WorldAccessError(name.into())),
    }
}
pub(super) fn field_edit(chunk: [u8; 4], record: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
