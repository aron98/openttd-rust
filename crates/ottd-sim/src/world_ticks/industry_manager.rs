// SPDX-License-Identifier: GPL-2.0-only
//! Saved fund-only industry manager from OpenTTD 15.3 industry_cmd.cpp:2632-2743,3086-3132.
use super::{State, WorldTickError, house_cargo::random, unsupported};

const INDUSTRY_TYPES: usize = 240;

#[derive(Clone, Copy, Default)]
struct BuildData {
    probability: u32,
    minimum: u8,
    target: u16,
    wait: u16,
}

pub(super) fn daily(state: &mut State<'_>) -> Result<(), WorldTickError> {
    let increment = state.width.saturating_mul(state.height) / 31;
    let counter = u32::try_from(state.number(b"ECMY", 0, "industry_daily_change_counter")?)
        .map_err(|_| unsupported("industry_day", "counter"))?
        .wrapping_add(increment);
    state.set_number(
        b"ECMY",
        0,
        "industry_daily_change_counter",
        i64::from(counter & 0xffff),
    )?;
    let attempts = counter >> 16;
    if attempts == 0 {
        return Ok(());
    }
    if state.number(b"PATS", 0, "difficulty.industry_density")? != 0 {
        return Err(unsupported(
            "industry_day",
            "automatic industry creation settings",
        ));
    }
    let wanted = u32::try_from(state.number(b"IBLD", 0, "wanted_inds")?)
        .map_err(|_| unsupported("industry_day", "wanted industries"))?
        >> 16;
    let percent = 3_u32.saturating_add(wanted).min(9);
    for _ in 0..attempts {
        let chance = (random(state)? & 0xffff)
            .saturating_mul(100)
            .saturating_add(50)
            >> 16;
        if chance < percent {
            try_build(state, wanted)?;
        }
        // Industry::GetRandom returns nullptr without drawing for the admitted empty pool.
    }
    Ok(())
}

fn try_build(state: &mut State<'_>, wanted: u32) -> Result<(), WorldTickError> {
    let mut rows = [BuildData::default(); INDUSTRY_TYPES];
    if state.table(b"ITBL")?.records().count() != INDUSTRY_TYPES {
        return Err(unsupported(
            "industry_builder",
            "industry type table membership",
        ));
    }
    let mut changed = false;
    let mut planned = 0_u32;
    for (index, row) in rows.iter_mut().enumerate() {
        let id = u32::try_from(index).map_err(|_| unsupported("industry_builder", "type"))?;
        row.probability = u32::try_from(state.number(b"ITBL", id, "probability")?)
            .map_err(|_| unsupported("industry_builder", "probability"))?;
        row.minimum = u8::try_from(state.number(b"ITBL", id, "min_number")?)
            .map_err(|_| unsupported("industry_builder", "minimum"))?;
        row.target = u16::try_from(state.number(b"ITBL", id, "target_count")?)
            .map_err(|_| unsupported("industry_builder", "target"))?;
        row.wait = u16::try_from(state.number(b"ITBL", id, "wait_count")?)
            .map_err(|_| unsupported("industry_builder", "wait"))?;
        changed |= row.probability != 0 || row.minimum != 0;
        planned = planned.wrapping_add(u32::from(row.target));
    }
    changed |= planned != wanted;
    // GetIndustryGamePlayProbability returns probability/minimum zero for fund-only
    // before consulting industry specifications. A changed plan has total probability zero.
    for row in &mut rows {
        row.probability = 0;
        row.minimum = 0;
        if changed {
            row.target = 0;
        }
    }
    if rows.iter().any(|row| row.target > 0 && row.wait == 0) {
        return Err(unsupported(
            "industry_builder",
            "reached original PlaceIndustry",
        ));
    }
    for (index, row) in rows.iter().enumerate() {
        let id = u32::try_from(index).map_err(|_| unsupported("industry_builder", "type"))?;
        for (field, value) in [
            ("probability", i64::from(row.probability)),
            ("min_number", i64::from(row.minimum)),
            ("target_count", i64::from(row.target)),
            ("wait_count", i64::from(row.wait.saturating_sub(1))),
        ] {
            state.set_number(b"ITBL", id, field, value)?;
        }
    }
    Ok(())
}
