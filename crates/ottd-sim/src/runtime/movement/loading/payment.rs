//! Native empty `CargoPayment` construction/destruction with tentative pool history.
use super::{RoadPhases, State, WorldTickError, number, records, unsupported};
use crate::runtime::RuntimeError;
use ottd_save::{WireValue, world::WorldEdit};
pub(super) fn create(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
    station: u16,
) -> Result<(), WorldTickError> {
    if !runner.transport.payment_stations.is_empty() {
        return Err(unsupported("road_loading", "existing cargo payment"));
    }
    let id = runner
        .transport
        .payments
        .allocate()
        .map_err(RuntimeError::from)?;
    let row = records::construct(
        state.table(b"CAPY")?.schema(),
        &[
            (
                "front",
                WireValue::Unsigned(u64::from(runner.id.raw()).wrapping_add(1)),
            ),
            ("route_profit", WireValue::Signed(0)),
            ("visual_profit", WireValue::Signed(0)),
            ("visual_transfer", WireValue::Signed(0)),
        ],
    )?;
    state.apply(WorldEdit::InsertRecord {
        chunk: *b"CAPY",
        record: id,
        value: row,
    })?;
    runner.transport.payment_stations.insert(id, station);
    Ok(())
}
pub(super) fn delete(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    let Some((&id, &station)) = runner.transport.payment_stations.first_key_value() else {
        return Ok(());
    };
    if runner.transport.payment_stations.len() != 1
        || i64::from(station) != number(state, runner.id, false, "last_station_visited")?
    {
        return Err(unsupported("road_loading", "payment station cache"));
    }
    for name in ["route_profit", "visual_profit", "visual_transfer"] {
        if state.number(b"CAPY", id, name)? != 0 {
            return Err(unsupported("road_loading", "nonzero cargo payment"));
        }
    }
    if state.number(b"CAPY", id, "front")? != i64::from(runner.id.raw()).wrapping_add(1) {
        return Err(unsupported("road_loading", "payment front"));
    }
    state.apply(WorldEdit::RemoveRecord {
        chunk: *b"CAPY",
        record: id,
    })?;
    runner
        .transport
        .payments
        .free(id)
        .map_err(RuntimeError::from)?;
    runner.transport.payment_stations.remove(&id);
    Ok(())
}
