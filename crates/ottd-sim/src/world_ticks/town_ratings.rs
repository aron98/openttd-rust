//! Native monthly town ratings where every KD visitation order avoids int16 saturation.
use super::{State, WorldTickError, access::number, unsupported};
use crate::runtime::stations::saved::StationRow;
use ottd_save::WireValue;
use std::collections::BTreeMap;

pub(super) fn update(
    state: &State<'_>,
    town: u32,
    ratings: &mut [WireValue],
) -> Result<(), WorldTickError> {
    let companies = state.company_ids();
    let steps = station_steps(state, town)?;
    for (index, value) in ratings.iter_mut().enumerate() {
        let mut rating = number(value)?;
        let company =
            u32::try_from(index).map_err(|_| unsupported("town_month", "company index"))?;
        if companies.contains(&company) && rating < 200 {
            rating = rating.saturating_add(5).min(200);
        }
        if let Some((positive, negative)) = steps.get(&company) {
            rating = apply_steps(rating, *positive, *negative)?;
        }
        *value = WireValue::Signed(rating.clamp(-1000, 1000));
    }
    Ok(())
}
fn apply_steps(rating: i64, positive: i64, negative: i64) -> Result<i64, WorldTickError> {
    let upper = rating
        .checked_add(positive)
        .ok_or_else(|| unsupported("town_month", "rating sum"))?;
    let lower = rating
        .checked_add(negative)
        .ok_or_else(|| unsupported("town_month", "rating sum"))?;
    if upper > i64::from(i16::MAX) || lower < i64::from(i16::MIN) {
        return Err(unsupported(
            "town_month",
            "historical station KD order at int16 saturation",
        ));
    }
    Ok(upper.wrapping_add(negative))
}
fn station_steps(
    state: &State<'_>,
    town: u32,
) -> Result<BTreeMap<u32, (i64, i64)>, WorldTickError> {
    let mut steps = BTreeMap::<u32, (i64, i64)>::new();
    let table = state.table(b"STNN")?;
    if table.records().next().is_none() {
        return Ok(steps);
    }
    let town_id = u16::try_from(town).map_err(|_| unsupported("town_month", "town identifier"))?;
    let radius = *state
        .towns
        .as_ref()
        .ok_or_else(|| unsupported("town_month", "town radius cache"))?
        .town(town_id)?
        .zone_radii
        .first()
        .ok_or_else(|| unsupported("town_month", "outer radius"))?;
    let center = u32::try_from(state.number(b"CITY", town, "xy")?)
        .map_err(|_| unsupported("town_month", "town sign"))?;
    let width = std::num::NonZeroU32::new(state.width)
        .ok_or_else(|| unsupported("town_month", "map width"))?;
    let (cx, cy) = (center % width, center / width);
    let search = radius / 2;
    let (left, top) = (cx.saturating_sub(search), cy.saturating_sub(search));
    let (right, bottom) = (
        cx.saturating_add(search).saturating_add(1).min(state.width),
        cy.saturating_add(search)
            .saturating_add(1)
            .min(state.height),
    );
    let companies = state.company_ids();
    for (_, record) in table.records() {
        let station = StationRow {
            schema: table.schema(),
            record,
        };
        // Station::Iterate excludes waypoints, but includes unused station signs.
        if station.number("facilities")? & 128 != 0 {
            continue;
        }
        let normal = station.child("normal")?;
        let base = normal.child("base")?;
        let xy = u32::try_from(base.number("xy")?)
            .map_err(|_| unsupported("town_month", "station sign"))?;
        let (x, y) = (xy % width, xy / width);
        if x < left || x >= right || y < top || y >= bottom {
            continue;
        }
        let (dx, dy) = (x.abs_diff(cx), y.abs_diff(cy));
        if dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) > radius {
            continue;
        }
        let owner = u32::try_from(base.number("owner")?)
            .map_err(|_| unsupported("town_month", "station owner"))?;
        if !companies.contains(&owner) {
            continue;
        }
        let entry = steps.entry(owner).or_default();
        if normal.number("time_since_load")? <= 20 || normal.number("time_since_unload")? <= 20 {
            entry.0 = entry.0.wrapping_add(12);
        } else {
            entry.1 = entry.1.wrapping_sub(15);
        }
    }
    Ok(steps)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_intermediate_is_bounded_not_only_the_final_sum() -> Result<(), WorldTickError> {
        assert_eq!(apply_steps(500, 12, -15)?, 497);
        assert_eq!(apply_steps(0, 30_000, -30_000)?, 0);
        assert!(apply_steps(0, 37_000, -37_000).is_err());
        assert!(apply_steps(1000, 32_000, -30_000).is_err());
        assert!(apply_steps(-1000, 30_000, -32_000).is_err());
        Ok(())
    }
}
