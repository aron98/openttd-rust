use super::{RuntimeError, VehicleId, view};
use ottd_save::{
    TableRecord, TableSchema, WireValue,
    world::{CandidateTable, PathElement, WorldEdit},
};
pub(super) fn field(id: VehicleId, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id.raw(),
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    }
}
fn indices(
    backup: view::Row<'_>,
    orders: Option<(&ottd_save::TableSchema, &[TableRecord])>,
) -> Result<(u64, u64), RuntimeError> {
    let mut real = backup.number("cur_real_order_index")?;
    let mut implicit = backup.number("cur_implicit_order_index")?;
    let mut kinds = Vec::new();
    if let Some((schema, rows)) = orders {
        for record in rows {
            kinds.push((view::Row { schema, record }).number("type")? & 15);
        }
    }
    let count =
        u64::try_from(kinds.len()).map_err(|_| RuntimeError::Invalid("restore order count"))?;
    if real >= count {
        real = 0;
    }
    if kinds.iter().any(|kind| *kind != 8) {
        while kinds
            .get(usize::try_from(real).map_err(|_| RuntimeError::Invalid("restore real index"))?)
            == Some(&8)
        {
            real = real.saturating_add(1);
            if real >= count {
                real = 0;
            }
        }
    } else {
        real = 0;
    }
    if implicit >= count {
        implicit = real;
    }
    Ok((real, implicit))
}
pub(super) fn copy(
    backup: view::Row<'_>,
    current: view::Row<'_>,
    vehicles: CandidateTable<'_>,
    vehicle: VehicleId,
    orders: Option<(&TableSchema, &[TableRecord])>,
) -> Result<Vec<WorldEdit>, RuntimeError> {
    let mut edits = Vec::new();
    let (real, implicit) = indices(backup, orders)?;
    let name = backup.field("name")?;
    let mut unique = true;
    for (_, record) in vehicles.records() {
        if let Some((_, row)) = view::vehicle(view::Row {
            schema: vehicles.schema(),
            record,
        })? {
            if row.field("name")? == name {
                unique = false;
            }
        }
    }
    edits.push(field(
        vehicle,
        "name",
        if unique {
            name.clone()
        } else {
            WireValue::Bytes(Vec::new())
        },
    ));
    for name in ["lateness_counter", "timetable_start", "service_interval"] {
        edits.push(field(vehicle, name, backup.field(name)?.clone()));
    }
    let bits = u32::try_from(backup.number("current_order_time")?)
        .map_err(|_| RuntimeError::Invalid("restore current order time"))?;
    edits.push(field(
        vehicle,
        "current_order_time",
        WireValue::Signed(i64::from(i32::from_le_bytes(bits.to_le_bytes()))),
    ));
    let flags = backup.number("vehicle_flags")?;
    edits.push(field(
        vehicle,
        "vehicle_flags",
        WireValue::Unsigned((current.number("vehicle_flags")? & !0x20) | (flags & 0x338)),
    ));
    edits.push(field(
        vehicle,
        "cur_real_order_index",
        WireValue::Unsigned(real),
    ));
    edits.push(field(
        vehicle,
        "cur_implicit_order_index",
        WireValue::Unsigned(implicit),
    ));
    Ok(edits)
}
