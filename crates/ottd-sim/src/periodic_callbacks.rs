//! Original periodic bookkeeping state transitions.
use crate::{CompanyCallbacks, HouseCallbacks, StationCallbacks};

/// Rejected incomplete or unsupported callback state.
#[derive(Debug, thiserror::Error)]
pub enum PeriodicCallbackError {
    /// Financial windows and sounds are outside the headless state boundary.
    #[error("company yearly callback requires show_finances=false")]
    Finances,
    /// Native map, pool, or cargo array invariants were violated.
    #[error("invalid callback map dimensions, pool IDs, or cargo slot count")]
    State,
}

/// Increments completed-house ages, saturating at 255, over the whole map.
/// # Errors
/// Rejects invalid dimensions, tile counts, and unknown native tile types.
pub fn run_house_year(mut state: HouseCallbacks) -> Result<HouseCallbacks, PeriodicCallbackError> {
    let dimensions = ottd_core::MapDimensions::new(state.map.width, state.map.height)
        .map_err(|_| PeriodicCallbackError::State)?;
    if u32::try_from(state.map.tiles.len()).ok() != Some(dimensions.tile_count())
        || state.map.tiles.iter().any(|tile| tile.tile_type >> 4 > 10)
    {
        return Err(PeriodicCallbackError::State);
    }
    for tile in &mut state.map.tiles {
        if tile.tile_type >> 4 == 3 && tile.m3 & 128 != 0 {
            tile.m5 = tile.m5.saturating_add(1);
        }
    }
    Ok(state)
}

/// Rolls all company expense tables forward by one year.
/// # Errors
/// Rejects enabled financial UI or duplicate, unordered, out-of-range IDs.
pub fn run_company_year(
    mut state: CompanyCallbacks,
) -> Result<CompanyCallbacks, PeriodicCallbackError> {
    if state.show_finances {
        return Err(PeriodicCallbackError::Finances);
    }
    let mut previous = None;
    for company in &state.companies {
        if company.id >= 15 || previous.is_some_and(|id| id >= company.id) {
            return Err(PeriodicCallbackError::State);
        }
        previous = Some(company.id);
    }
    for company in &mut state.companies {
        company.yearly_expenses.rotate_right(1);
        if let Some(current) = company.yearly_expenses.first_mut() {
            current.fill(0);
        }
    }
    Ok(state)
}

/// Transfers current-month acceptance into last-month acceptance for every cargo.
/// # Errors
/// Rejects duplicate, unordered, out-of-range station IDs or incomplete cargo arrays.
pub fn run_station_month(
    mut state: StationCallbacks,
) -> Result<StationCallbacks, PeriodicCallbackError> {
    let mut previous = None;
    for station in &state.stations {
        if station.id >= 64_000
            || previous.is_some_and(|id| id >= station.id)
            || station.goods.len() != 64
        {
            return Err(PeriodicCallbackError::State);
        }
        previous = Some(station.id);
    }
    for station in &mut state.stations {
        for cargo in &mut station.goods {
            cargo.status = (cargo.status & !0x18) | ((cargo.status & 0x10) >> 1);
        }
    }
    Ok(state)
}
