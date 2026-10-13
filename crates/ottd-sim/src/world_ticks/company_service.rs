//! Service-station component of `UpdateCompanyRatingAndValue`.
use super::super::{State, WorldTickError, unsupported};
use crate::runtime::stations::saved::StationRow;

pub(super) fn facilities(state: &State<'_>, owner: u32) -> Result<i64, WorldTickError> {
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
    Ok(i64::from(count))
}

#[cfg(test)]
#[path = "company_service_tests.rs"]
mod tests;
