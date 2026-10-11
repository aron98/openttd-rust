//! Sparse station and transport row adapters; saved records remain authoritative.
use super::{State, WorldTickError, unsupported};
use crate::runtime::stations::saved::StationRow;
use ottd_save::{
    TableRecord, TableSchema, WireValue,
    world::{PathElement, WorldEdit},
};

pub(super) fn row<'a>(
    state: &'a State<'_>,
    key: [u8; 4],
    id: u32,
) -> Result<StationRow<'a>, WorldTickError> {
    let table = state.table(&key)?;
    Ok(StationRow {
        schema: table.schema(),
        record: table
            .record(id)
            .ok_or_else(|| unsupported("transport", "missing row"))?,
    })
}
pub(super) fn normal<'a>(state: &'a State<'_>, id: u32) -> Result<StationRow<'a>, WorldTickError> {
    Ok(row(state, *b"STNN", id)?.child("normal")?)
}
pub(super) fn station_field(
    state: &mut State<'_>,
    id: u32,
    field: (&str, WireValue),
) -> Result<(), WorldTickError> {
    if let WireValue::Array(values) = field.1 {
        return state.apply(WorldEdit::ArrayList {
            chunk: *b"STNN",
            record: id,
            path: vec![
                PathElement::Field("normal".into()),
                PathElement::Index(0),
                PathElement::Field(field.0.into()),
            ],
            values,
        });
    }
    state.apply(WorldEdit::Field {
        chunk: *b"STNN",
        record: id,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field(field.0.into()),
        ],
        value: field.1,
    })
}
pub(super) fn schedule(
    state: &mut State<'_>,
    name: &str,
    values: Vec<WireValue>,
) -> Result<(), WorldTickError> {
    state.apply(WorldEdit::ArrayList {
        chunk: *b"LGRS",
        record: 0,
        path: vec![PathElement::Field(name.into())],
        values,
    })
}
pub(super) fn goods_field(
    state: &mut State<'_>,
    station_cargo: (u32, u8),
    field: (&str, WireValue),
) -> Result<(), WorldTickError> {
    state.apply(WorldEdit::Field {
        chunk: *b"STNN",
        record: station_cargo.0,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field("goods".into()),
            PathElement::Index(usize::from(station_cargo.1)),
            PathElement::Field(field.0.into()),
        ],
        value: field.1,
    })
}
pub(super) fn goods<'a>(
    state: &'a State<'_>,
    station: u32,
    cargo: u8,
) -> Result<StationRow<'a>, WorldTickError> {
    let (schema, rows) = normal(state, station)?.rows("goods")?;
    Ok(StationRow {
        schema,
        record: rows
            .get(usize::from(cargo))
            .ok_or_else(|| unsupported("transport", "cargo row"))?,
    })
}
pub(super) fn construct(
    schema: &TableSchema,
    fields: &[(&str, WireValue)],
) -> Result<TableRecord, WorldTickError> {
    schema
        .fields()
        .iter()
        .map(|field| {
            fields
                .iter()
                .find(|(name, _)| *name == field.name())
                .map(|(_, value)| value.clone())
                .ok_or_else(|| unsupported("transport", "constructor schema field"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(TableRecord::new)
}
pub(super) fn child<'a>(
    schema: &'a TableSchema,
    name: &str,
) -> Result<&'a TableSchema, WorldTickError> {
    schema
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .and_then(ottd_save::FieldSchema::child)
        .ok_or_else(|| unsupported("transport", "constructor child schema"))
}
pub(super) fn edit(
    schema: &TableSchema,
    record: &mut TableRecord,
    field: (&str, WireValue),
) -> Result<(), WorldTickError> {
    let index = schema
        .fields()
        .iter()
        .position(|item| item.name() == field.0)
        .ok_or_else(|| unsupported("transport", "record field"))?;
    *record
        .values_mut()
        .get_mut(index)
        .ok_or_else(|| unsupported("transport", "record position"))? = field.1;
    Ok(())
}
