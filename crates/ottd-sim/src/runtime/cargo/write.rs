//! Saved cargo-list publication inside the borrowed world candidate.
use super::{RuntimeError, StationList, VehicleList, read::Read};
use crate::{WorldTickError, WorldTickState as State, runtime::VehicleId};
use ottd_save::{
    TableRecord, WireValue,
    world::{PathElement, WorldEdit},
};

pub(super) fn station(
    state: &mut State<'_>,
    station: u32,
    cargo: u8,
    list: &StationList,
) -> Result<(), WorldTickError> {
    let normal = Read::Candidate(state.view())
        .row(*b"STNN", station)?
        .child("normal")?;
    let (schema, goods) = normal.rows("goods")?;
    let goods = crate::runtime::stations::saved::StationRow {
        schema,
        record: goods
            .get(usize::from(cargo))
            .ok_or(RuntimeError::Invalid("cargo goods row"))?,
    };
    let (schema, _) = goods.rows("cargo")?;
    let mut rows = Vec::new();
    for (next, packets) in &list.packets {
        let mut values = Vec::new();
        for field in schema.fields() {
            values.push(match field.name() {
                "first" => WireValue::Unsigned(u64::from(*next)),
                "second" => WireValue::Array(
                    packets
                        .iter()
                        .map(|id| WireValue::Unsigned(u64::from(*id) + 1))
                        .collect(),
                ),
                _ => return Err(RuntimeError::Invalid("station cargo list schema").into()),
            });
        }
        rows.push(TableRecord::new(values));
    }
    state.apply(WorldEdit::StructList {
        chunk: *b"STNN",
        record: station,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field("goods".into()),
            PathElement::Index(usize::from(cargo)),
            PathElement::Field("cargo".into()),
        ],
        rows,
    })
}

pub(super) fn vehicle(
    state: &mut State<'_>,
    vehicle: VehicleId,
    list: &VehicleList,
) -> Result<(), WorldTickError> {
    for (name, values) in [
        (
            "cargo.packets",
            list.packets
                .iter()
                .map(|id| WireValue::Unsigned(u64::from(*id) + 1))
                .collect(),
        ),
        (
            "cargo.action_counts",
            list.actions
                .iter()
                .map(|count| WireValue::Unsigned(u64::from(*count)))
                .collect(),
        ),
    ] {
        state.apply(WorldEdit::ArrayList {
            chunk: *b"VEHS",
            record: vehicle.raw(),
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("common".into()),
                PathElement::Index(0),
                PathElement::Field(name.into()),
            ],
            values,
        })?;
    }
    Ok(())
}

pub(super) fn goods<'a>(
    state: &'a State<'_>,
    station: u32,
    cargo: u8,
) -> Result<crate::runtime::stations::saved::StationRow<'a>, WorldTickError> {
    let normal = Read::Candidate(state.view())
        .row(*b"STNN", station)?
        .child("normal")?;
    let (schema, rows) = normal.rows("goods")?;
    Ok(crate::runtime::stations::saved::StationRow {
        schema,
        record: rows
            .get(usize::from(cargo))
            .ok_or(RuntimeError::Invalid("cargo goods row"))?,
    })
}
pub(super) fn goods_field(
    state: &mut State<'_>,
    station: u32,
    cargo: u8,
    name: &str,
    value: u64,
) -> Result<(), WorldTickError> {
    state.apply(WorldEdit::Field {
        chunk: *b"STNN",
        record: station,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field("goods".into()),
            PathElement::Index(usize::from(cargo)),
            PathElement::Field(name.into()),
        ],
        value: WireValue::Unsigned(value),
    })
}
