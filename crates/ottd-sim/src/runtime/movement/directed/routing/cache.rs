//! Authoritative road path `StructList`, in native backtracking order.
use super::super::{State, VehicleId};
use super::{WorldTickError, network::Position, unsupported};
use crate::runtime::stations::saved::StationRow;
use ottd_save::{
    TableRecord, WireValue,
    world::{PathElement, WorldEdit},
};
fn road<'a>(state: &'a State<'_>, id: VehicleId) -> Result<StationRow<'a>, WorldTickError> {
    let table = state.table(b"VEHS")?;
    Ok(StationRow {
        schema: table.schema(),
        record: table
            .record(id.raw())
            .ok_or_else(|| unsupported("road_yapf", "vehicle row"))?,
    }
    .child("roadveh")?)
}
pub(super) fn read(state: &State<'_>, id: VehicleId) -> Result<Vec<Position>, WorldTickError> {
    let (schema, rows) = road(state, id)?.rows("path")?;
    rows.iter()
        .map(|record| {
            let row = StationRow { schema, record };
            Ok(Position {
                tile: u32::try_from(row.number("tile")?)
                    .map_err(|_| unsupported("road_yapf", "cached tile"))?,
                track: u8::try_from(row.number("trackdir")?)
                    .map_err(|_| unsupported("road_yapf", "cached track"))?,
            })
        })
        .collect()
}
pub(super) fn write(
    state: &mut State<'_>,
    id: VehicleId,
    path: &[Position],
) -> Result<(), WorldTickError> {
    let (schema, _) = road(state, id)?.rows("path")?;
    let rows = path
        .iter()
        .map(|position| {
            schema
                .fields()
                .iter()
                .map(|field| match field.name() {
                    "tile" => Ok(WireValue::Unsigned(u64::from(position.tile))),
                    "trackdir" => Ok(WireValue::Unsigned(u64::from(position.track))),
                    _ => Err(unsupported("road_yapf", "path schema field")),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(TableRecord::new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    state.apply(WorldEdit::StructList {
        chunk: *b"VEHS",
        record: id.raw(),
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("path".into()),
        ],
        rows,
    })
}
