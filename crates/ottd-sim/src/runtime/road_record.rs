#[cfg(test)]
mod tests;
use super::{RoadBuildState, RuntimeError};
use ottd_save::{TableRecord, TableSchema, WireValue};
use std::collections::BTreeMap;

fn common(
    schema: &TableSchema,
    state: RoadBuildState,
    cargo_paid_for: u16,
) -> Result<TableRecord, RuntimeError> {
    use WireValue::{Array as A, Bytes as B, Signed as S, Unsigned as U};
    bind(
        schema,
        vec![
            ("subtype", U(1)),
            ("next", U(0)),
            ("name", B(Vec::new())),
            ("unitnumber", U(u64::from(state.unit))),
            ("owner", U(u64::from(state.owner))),
            ("tile", U(u64::from(state.tile))),
            ("dest_tile", U(u64::from(u32::MAX))),
            ("x_pos", U(u64::from(state.x))),
            ("y_pos", U(u64::from(state.y))),
            ("z_pos", S(i64::from(state.z))),
            ("direction", U(u64::from(state.direction))),
            ("spritenum", U(u64::from(state.image))),
            ("engine_type", U(u64::from(state.engine))),
            ("cur_speed", U(0)),
            ("subspeed", U(0)),
            ("acceleration", U(0)),
            ("motion_counter", U(0)),
            ("progress", U(0)),
            ("vehstatus", U(11)),
            ("last_station_visited", U(65535)),
            ("last_loading_station", U(65535)),
            ("cargo_type", U(u64::from(state.cargo))),
            ("cargo_subtype", U(0)),
            ("cargo_cap", U(u64::from(state.capacity))),
            ("refit_cap", U(0)),
            ("cargo.packets", A(Vec::new())),
            ("cargo.action_counts", A(vec![U(0); 4])),
            ("cargo_age_counter", U(1)),
            ("day_counter", U(0)),
            ("tick_counter", U(0)),
            ("running_ticks", U(0)),
            ("cur_implicit_order_index", U(0)),
            ("cur_real_order_index", U(0)),
            ("current_order.type", U(0)),
            ("current_order.flags", U(0)),
            ("current_order.dest", U(0)),
            ("current_order.refit_cargo", U(254)),
            ("current_order.wait_time", U(0)),
            ("current_order.travel_time", U(0)),
            ("current_order.max_speed", U(65535)),
            ("timetable_start", U(0)),
            ("orders", U(0)),
            ("age", S(0)),
            ("economy_age", S(0)),
            ("max_age", S(i64::from(state.max_age))),
            ("date_of_last_service", S(i64::from(state.economy_date))),
            (
                "date_of_last_service_newgrf",
                S(i64::from(state.calendar_date)),
            ),
            ("service_interval", U(u64::from(state.service_interval))),
            ("reliability", U(u64::from(state.reliability))),
            ("reliability_spd_dec", U(u64::from(state.reliability_decay))),
            ("breakdown_ctr", U(0)),
            ("breakdown_delay", U(0)),
            ("breakdowns_since_last_service", U(0)),
            ("breakdown_chance", U(0)),
            ("build_year", S(i64::from(state.build_year))),
            ("load_unload_ticks", U(0)),
            ("cargo_paid_for", U(u64::from(cargo_paid_for))),
            (
                "vehicle_flags",
                U((u64::from(state.preview) << 2) | (u64::from(state.service_percent) << 9)),
            ),
            ("profit_this_year", S(0)),
            ("profit_last_year", S(0)),
            ("value", S(state.value)),
            ("random_bits", U(u64::from(state.random_bits))),
            ("waiting_triggers", U(0)),
            ("next_shared", U(0)),
            ("group_id", U(65534)),
            ("current_order_time", S(0)),
            ("last_loading_tick", U(0)),
            ("lateness_counter", S(0)),
            ("depot_unbunching_last_departure", U(0)),
            ("depot_unbunching_next_departure", U(0)),
            ("round_trip_time", S(0)),
        ],
    )
}
pub(super) fn build(
    schema: &TableSchema,
    state: RoadBuildState,
    cargo_paid_for: u16,
) -> Result<TableRecord, RuntimeError> {
    use WireValue::{Structs as L, Unsigned as U};
    let road_schema = child(schema, "roadveh")?;
    let common = common(child(road_schema, "common")?, state, cargo_paid_for)?;
    let road = bind(
        road_schema,
        vec![
            ("common", L(vec![common])),
            ("state", U(254)),
            ("frame", U(0)),
            ("blocked_ctr", U(0)),
            ("overtaking", U(0)),
            ("overtaking_ctr", U(0)),
            ("crashed_ctr", U(0)),
            ("reverse_ctr", U(0)),
            ("path", L(Vec::new())),
            ("gv_flags", U(0)),
        ],
    )?;
    bind(
        schema,
        vec![
            ("type", U(1)),
            ("train", L(Vec::new())),
            ("roadveh", L(vec![road])),
            ("ship", L(Vec::new())),
            ("aircraft", L(Vec::new())),
            ("effect", L(Vec::new())),
            ("disaster", L(Vec::new())),
        ],
    )
}
fn child<'a>(schema: &'a TableSchema, name: &str) -> Result<&'a TableSchema, RuntimeError> {
    schema
        .fields()
        .iter()
        .find(|f| f.name() == name)
        .and_then(ottd_save::FieldSchema::child)
        .ok_or(RuntimeError::Invalid("road constructor child schema"))
}
fn bind(
    schema: &TableSchema,
    values: Vec<(&'static str, WireValue)>,
) -> Result<TableRecord, RuntimeError> {
    if schema.fields().len() != values.len() {
        return Err(RuntimeError::Invalid("road constructor field inventory"));
    }
    let mut values: BTreeMap<_, _> = values.into_iter().collect();
    let row = schema
        .fields()
        .iter()
        .map(|f| {
            values
                .remove(f.name())
                .ok_or(RuntimeError::Invalid("road constructor field name"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TableRecord::new(row))
}
