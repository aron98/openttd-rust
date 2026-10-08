//! Complete original-economy monthly industry callback within a closed live-industry domain.
use crate::{IndustryCallbacks, IndustryMonth, industry_history};

/// Rejected industry callback context or incomplete history state.
#[derive(Debug, thiserror::Error)]
pub enum IndustryCallbackError {
    /// Random production, closure, and `NewGRF` branches are outside this domain.
    #[error("industry monthly requires original economy, no NewGRF, and live original industries")]
    Context,
    /// Native pool, map, phase, or history invariants were violated.
    #[error("invalid industry map, IDs, callback phase, or 61-record history")]
    State,
}

/// Executes builder growth and complete statistics rollover in original economy.
///
/// The original native monthly production-change function returns immediately in
/// this domain. No closure, production policy, RNG, or news work is skipped.
/// # Errors
/// Rejects other economy types, custom industry specs, closure-marked industries,
/// invalid IDs/map/phase or incomplete histories before changing state.
pub fn run_industry_month(
    mut state: IndustryCallbacks,
    phase: IndustryMonth,
) -> Result<IndustryCallbacks, IndustryCallbackError> {
    if state.economy_type != 0
        || state.newgrf
        || state
            .industries
            .iter()
            .any(|i| i.prod_level == 0 || i.industry_type >= 37)
    {
        return Err(IndustryCallbackError::Context);
    }
    let dimensions = ottd_core::MapDimensions::new(state.map_width, state.map_height)
        .map_err(|_| IndustryCallbackError::State)?;
    if state.industry_density > 6 || phase.month >= 12 || !(0..=5_000_001).contains(&phase.year) {
        return Err(IndustryCallbackError::State);
    }
    let mut previous = None;
    for industry in &state.industries {
        if industry.id >= 64_000
            || previous.is_some_and(|id| id >= industry.id)
            || !(-1..=5_000_001).contains(&industry.last_prod_year)
            || industry.produced.iter().any(|p| p.history.len() != 61)
            || industry
                .accepted
                .iter()
                .any(|a| a.history.as_ref().is_some_and(|h| h.len() != 61))
        {
            return Err(IndustryCallbackError::State);
        }
        previous = Some(industry.id);
    }
    let scale = dimensions.tile_count() / 4096;
    let max_behind = 1_u32.saturating_add(3_u32.saturating_mul(scale).div_ceil(16).min(99));
    let count = u32::try_from(state.industries.len()).map_err(|_| IndustryCallbackError::State)?;
    if state.industry_density != 0 && count.saturating_add(max_behind) >= state.wanted_inds >> 16 {
        state.wanted_inds = state
            .wanted_inds
            .wrapping_add(1911_u32.saturating_mul(scale).div_ceil(16));
    }
    for industry in &mut state.industries {
        industry.valid_history =
            industry_history::update_valid(industry.valid_history, phase.month)?;
        for produced in &mut industry.produced {
            if produced.cargo == 255 {
                continue;
            }
            if produced.history.first().is_some_and(|h| h.production != 0) {
                industry.last_prod_year = phase.year;
            }
            industry_history::rotate(&mut produced.history, industry.valid_history, phase.month)?;
        }
        for accepted in &mut industry.accepted {
            if accepted.cargo == 255 {
                continue;
            }
            if let Some(history) = &mut accepted.history {
                let average = accepted
                    .accumulated_waiting
                    .checked_div(phase.days_since_last_month.max(1))
                    .ok_or(IndustryCallbackError::State)?
                    .min(u32::from(u16::MAX));
                history
                    .first_mut()
                    .ok_or(IndustryCallbackError::State)?
                    .waiting = u16::try_from(average).map_err(|_| IndustryCallbackError::State)?;
                accepted.accumulated_waiting = 0;
                industry_history::rotate(history, industry.valid_history, phase.month)?;
            }
        }
    }
    Ok(state)
}
