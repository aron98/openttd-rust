//! Service-station component of `UpdateCompanyRatingAndValue`.
use super::super::{State, WorldTickError, unsupported};
use crate::runtime::stations::saved::StationRow;

pub(super) fn score(state: &State<'_>, owner: u32) -> Result<i64, WorldTickError> {
    // The current vehicle consumer admits only primary single road vehicles.
    // Requiring every owned profit_last_year <= 0 proves both omitted vehicle
    // count and positive minimum-profit components are zero, at every age.
    let vehicles = state.table(b"VEHS")?;
    for (_, record) in vehicles.records() {
        let row = StationRow {
            schema: vehicles.schema(),
            record,
        }
        .child("roadveh")?
        .child("common")?;
        if row.number("owner")? == u64::from(owner)
            && State::number_value(crate::world_access::row_field(
                row.schema,
                row.record,
                "profit_last_year",
            )?)? > 0
        {
            return Err(unsupported(
                "company_quarter",
                "profitable vehicle score contribution",
            ));
        }
    }
    let stations = state.table(b"STNN")?;
    let mut count = 0_u32;
    for (_, record) in stations.records() {
        let station = StationRow {
            schema: stations.schema(),
            record,
        };
        let facilities = u8::try_from(station.number("facilities")?)
            .map_err(|_| unsupported("company_quarter", "station facility width"))?;
        if facilities & 128 != 0 {
            continue;
        }
        let normal = station.child("normal")?;
        if normal.child("base")?.number("owner")? == u64::from(owner)
            && (normal.number("time_since_load")? <= 20
                || normal.number("time_since_unload")? <= 20)
        {
            count = count.wrapping_add(facilities.count_ones());
        }
    }
    Ok(i64::from(count.min(80).wrapping_mul(100) / 80))
}

#[cfg(test)]
#[path = "company_service_tests.rs"]
mod tests;
