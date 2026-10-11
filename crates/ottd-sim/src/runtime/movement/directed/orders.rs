//! Ordinary manual station order selection from authoritative ORDL records.
use super::{SavedVehicleView, State, VehicleId, WorldTickError, number, set, unsupported};
use crate::runtime::stations::saved::StationRow;
use ottd_save::world::{PathElement, WorldEdit};

const FIELDS: [(&str, &str); 7] = [
    ("type", "current_order.type"),
    ("flags", "current_order.flags"),
    ("dest", "current_order.dest"),
    ("refit_cargo", "current_order.refit_cargo"),
    ("wait_time", "current_order.wait_time"),
    ("travel_time", "current_order.travel_time"),
    ("max_speed", "current_order.max_speed"),
];
fn selected(state: &State<'_>, id: VehicleId) -> Result<([i64; 7], usize), WorldTickError> {
    let list = u32::try_from(
        number(state, id, false, "orders")?
            .checked_sub(1)
            .ok_or_else(|| unsupported("road_orders", "missing list"))?,
    )
    .map_err(|_| unsupported("road_orders", "list pointer"))?;
    let table = state.table(b"ORDL")?;
    let row = StationRow {
        schema: table.schema(),
        record: table
            .record(list)
            .ok_or_else(|| unsupported("road_orders", "missing list"))?,
    };
    let (schema, rows) = row.rows("orders")?;
    if rows.is_empty() {
        return Err(unsupported("road_orders", "empty list"));
    }
    for record in rows {
        let order = StationRow { schema, record };
        if order.number("type")? != 33
            || order.number("flags")? != 0
            || order.number("refit_cargo")? != 254
            || order.number("wait_time")? != 0
            || order.number("travel_time")? != 0
        {
            return Err(unsupported(
                "road_orders",
                "ordinary untimetabled station orders required",
            ));
        }
    }
    let real = usize::try_from(number(state, id, false, "cur_real_order_index")?)
        .map_err(|_| unsupported("road_orders", "real index"))?;
    let real = if real < rows.len() { real } else { 0 };
    let implicit = usize::try_from(number(state, id, false, "cur_implicit_order_index")?)
        .map_err(|_| unsupported("road_orders", "implicit index"))?;
    if implicit >= rows.len() {
        return Err(unsupported("road_orders", "implicit index outside list"));
    }
    let order = StationRow {
        schema,
        record: rows
            .get(real)
            .ok_or_else(|| unsupported("road_orders", "real index outside list"))?,
    };
    let mut values = [0_i64; 7];
    for (value, (name, _)) in values.iter_mut().zip(FIELDS) {
        *value = i64::try_from(order.number(name)?)
            .map_err(|_| unsupported("road_orders", "order field"))?;
    }
    Ok((values, real))
}
pub(super) fn process(
    state: &mut State<'_>,
    id: VehicleId,
    content: &crate::content::ContentCatalog,
) -> Result<(), WorldTickError> {
    let (values, real) = selected(state, id)?;
    set(
        state,
        id,
        false,
        "cur_real_order_index",
        i64::try_from(real).map_err(|_| unsupported("road_orders", "index"))?,
    )?;
    let mut unchanged = number(state, id, false, "dest_tile")? != 0;
    for ((_, field), value) in FIELDS.iter().zip(values) {
        unchanged &= number(state, id, false, field)? == value;
    }
    if unchanged {
        return Ok(());
    }
    let station = u32::try_from(values[2]).map_err(|_| unsupported("road_orders", "station ID"))?;
    let table = state.table(b"STNN")?;
    let station_row = StationRow {
        schema: table.schema(),
        record: table
            .record(station)
            .ok_or_else(|| unsupported("road_orders", "missing destination"))?,
    };
    let normal = station_row.child("normal")?;
    let cargo = SavedVehicleView::candidate(state.view(), id)?.cargo_type()?;
    let spec = content
        .cargo()
        .get(usize::from(cargo))
        .ok_or_else(|| unsupported("road_orders", "cargo specification"))?;
    if spec.label == 0 {
        return Err(unsupported("road_orders", "invalid cargo"));
    }
    let facility = if spec.classes & 1 != 0 { 4 } else { 2 };
    if station_row.number("facilities")? & facility == 0 {
        return Err(unsupported(
            "road_orders",
            "destination without matching facility",
        ));
    }
    let destination = i64::try_from(normal.child("base")?.number("xy")?)
        .map_err(|_| unsupported("road_orders", "destination tile"))?;
    for ((_, field), value) in FIELDS.iter().zip(values) {
        set(state, id, false, field, value)?;
    }
    if number(state, id, false, "last_station_visited")? == i64::from(station) {
        set(state, id, false, "last_station_visited", 65535)?;
    }
    if number(state, id, false, "dest_tile")? != destination {
        state.apply(WorldEdit::StructList {
            chunk: *b"VEHS",
            record: id.raw(),
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("path".into()),
            ],
            rows: Vec::new(),
        })?;
        set(state, id, false, "dest_tile", destination)?;
    }
    Ok(())
}
