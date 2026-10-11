//! Native quarterly asset contribution in the admitted single-road transport domain.
use super::{SavedVehicleView, State, VehicleId, WorldTickError, unsupported};
use crate::{
    content::{ContentCatalog, Price},
    runtime::stations::saved::StationRow,
};

pub(super) fn assets(
    state: &State<'_>,
    content: &ContentCatalog,
    owner: u32,
) -> Result<i64, WorldTickError> {
    let stations = state.table(b"STNN")?;
    let mut facilities = 0_u32;
    for (_, record) in stations.records() {
        let row = StationRow {
            schema: stations.schema(),
            record,
        };
        let mask = u32::try_from(row.number("facilities")?)
            .map_err(|_| unsupported("company_assets", "facility flags"))?;
        // Station::Iterate excludes waypoints; station admission separately bounds facilities.
        if mask & 0x80 != 0 {
            continue;
        }
        if row.child("normal")?.child("base")?.number("owner")? == u64::from(owner) {
            facilities = facilities.wrapping_add(mask.count_ones());
        }
    }
    let station_value = i64::from(facilities)
        .saturating_mul(content.prices().get(Price::StationValue))
        .saturating_mul(25);
    let mut value = station_value;
    // Road vehicles contribute each physical part, unlike aircraft's normal-front filter.
    // This controller admits exactly one road object and rejects articulated parts.
    for (id, _) in state.table(b"VEHS")?.records() {
        let vehicle = SavedVehicleView::candidate(state.view(), VehicleId::new(id))?;
        let vehicle_value = if u32::from(vehicle.owner()?) == owner {
            i64::try_from(vehicle.common_number("value")?)
                .map_err(|_| unsupported("company_assets", "road value"))?
                .saturating_mul(3)
                >> 1
        } else {
            0
        };
        value = value.saturating_add(vehicle_value);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ottd_save::{Savegame, world::World};
    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn world(variable: &str) -> Result<World> {
        Ok(World::decode(&Savegame::decode(
            &std::fs::read(std::env::var(variable)?)?,
            ottd_save::DEFAULT_MAX_BYTES,
        )?)?)
    }
    #[test]
    #[ignore = "requires original command-created before/after quarterly snapshots"]
    fn road_station_assets_match_original_quarter() -> Result {
        // Given: original saved state immediately before a real quarter boundary.
        let mut before = world("ROAD_QUARTER_BEFORE")?;
        let native = world("ROAD_QUARTER_AFTER")?;
        let content = ContentCatalog::from_world(&before)?;
        let state = State::load(&mut before);
        // When: value every admitted native physical road object and station facility.
        let assets = assets(&state, &content, 0)?;
        let value = assets
            .saturating_sub(state.number(b"PLYR", 0, "current_loan")?)
            .saturating_add(state.number(b"PLYR", 0, "money")?)
            .max(1);
        // Then: the native quarter's published company value is exactly reproduced.
        let company = crate::runtime::stations::saved::row(&native, *b"PLYR", 0)?;
        let (schema, rows) = company.rows("old_economy")?;
        let expected = State::number_value(crate::world_access::row_field(
            schema,
            rows.first().ok_or("native quarter")?,
            "company_value",
        )?)?;
        std::fs::write(
            std::env::var("ROAD_QUARTER_RESULT")?,
            serde_json::to_vec_pretty(
                &serde_json::json!({"assets":assets,"rust_value":value,"native_value":expected,"equal":value==expected}),
            )?,
        )?;
        assert_eq!(value, expected);
        Ok(())
    }
}
