//! Calendar aging and yearly profit accounting from OpenTTD 15.3.
use crate::{VehicleCallbacks, VehicleKind, VehicleOperation};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DATE: i32 = 1_826_212_865;
const ALL_GROUP: u16 = 65_533;
const DEFAULT_GROUP: u16 = 65_534;

/// Rejection before any callback mutation.
#[derive(Debug, thiserror::Error)]
pub enum VehicleCallbackError {
    /// News and AI branches require subsystems outside this boundary.
    #[error("vehicle callbacks require human companies and disabled vehicle warnings")]
    Context,
    /// IDs, ages, membership, or scheduling are inconsistent.
    #[error("invalid vehicle pool, group membership, age, or calendar fraction")]
    State,
}

fn validate(
    state: &VehicleCallbacks,
    operation: VehicleOperation,
) -> Result<(), VehicleCallbackError> {
    if state.old_vehicle_warn || state.vehicle_income_warn {
        return Err(VehicleCallbackError::Context);
    }
    match operation {
        VehicleOperation::CalendarDay { date_fract } if date_fract >= 74 => {
            return Err(VehicleCallbackError::State);
        }
        VehicleOperation::CalendarDay { .. } | VehicleOperation::EconomyYear => {}
    }
    let owners: BTreeSet<_> = state.human_companies.iter().copied().collect();
    if owners.len() != state.human_companies.len() || owners.iter().any(|id| *id >= 15) {
        return Err(VehicleCallbackError::State);
    }
    let groups: BTreeSet<_> = state
        .groups
        .iter()
        .map(|g| (g.owner, g.kind, g.id))
        .collect();
    if groups.len() != state.groups.len()
        || state.groups.iter().any(|g| {
            !owners.contains(&g.owner)
                || !(g.id < 64_000 || g.id == ALL_GROUP || g.id == DEFAULT_GROUP)
        })
    {
        return Err(VehicleCallbackError::State);
    }
    let mut ordinary_ids = BTreeSet::new();
    if state
        .groups
        .iter()
        .any(|group| group.id < 64_000 && !ordinary_ids.insert(group.id))
    {
        return Err(VehicleCallbackError::State);
    }
    for owner in &owners {
        for kind in [
            VehicleKind::Train,
            VehicleKind::Road,
            VehicleKind::Ship,
            VehicleKind::Aircraft,
        ] {
            for id in [ALL_GROUP, DEFAULT_GROUP] {
                if !groups.contains(&(*owner, kind, id)) {
                    return Err(VehicleCallbackError::State);
                }
            }
        }
    }
    let mut previous = None;
    for vehicle in &state.vehicles {
        if previous.is_some_and(|id| id >= vehicle.id)
            || vehicle.id >= 0xff000
            || !owners.contains(&vehicle.owner)
            || ![vehicle.age, vehicle.max_age, vehicle.economy_age]
                .iter()
                .all(|age| (0..=MAX_DATE).contains(age))
            || vehicle.group_id == ALL_GROUP
            || !groups.contains(&(vehicle.owner, vehicle.kind, vehicle.group_id))
            || !groups.contains(&(vehicle.owner, vehicle.kind, ALL_GROUP))
        {
            return Err(VehicleCallbackError::State);
        }
        previous = Some(vehicle.id);
    }
    Ok(())
}

/// Runs the original callback's supported state effects after validating the boundary.
///
/// No movement, economy-day operation, news, or AI queues are implied. Calendar
/// aging invokes the native subtype gates; yearly profits retain fixed-point bits.
/// # Errors
/// Rejects unsupported context, inconsistent object references, or invalid ages.
pub fn run_vehicle_callback(
    mut state: VehicleCallbacks,
    operation: VehicleOperation,
) -> Result<VehicleCallbacks, VehicleCallbackError> {
    validate(&state, operation)?;
    match operation {
        VehicleOperation::CalendarDay { date_fract } => {
            for vehicle in &mut state.vehicles {
                if vehicle.id % 74 != u32::from(date_fract) {
                    continue;
                }
                let ages = match vehicle.kind {
                    VehicleKind::Train | VehicleKind::Ship => true,
                    VehicleKind::Road | VehicleKind::Aircraft => vehicle.primary(),
                };
                if !ages {
                    continue;
                }
                vehicle.age = vehicle.age.saturating_add(1).min(MAX_DATE);
                let reliability_ages = vehicle.primary()
                    || matches!(vehicle.kind, VehicleKind::Train) && vehicle.subtype & 8 != 0;
                if reliability_ages
                    && [0, 366, 731, 1096, 1461]
                        .contains(&vehicle.age.saturating_sub(vehicle.max_age))
                {
                    vehicle.reliability_spd_dec = vehicle.reliability_spd_dec.wrapping_mul(2);
                }
            }
        }
        VehicleOperation::EconomyYear => {
            for vehicle in &mut state.vehicles {
                if vehicle.primary() {
                    vehicle.profit_last_year = vehicle.profit_this_year;
                    vehicle.profit_this_year = 0;
                }
            }
            let mut groups: BTreeMap<_, _> = state
                .groups
                .iter_mut()
                .map(|group| ((group.owner, group.kind, group.id), group))
                .collect();
            for group in groups.values_mut() {
                group.profit_last_year = 0;
                group.profit_last_year_min_age = 0;
                group.num_vehicle_min_age = 0;
            }
            for vehicle in &state.vehicles {
                if !vehicle.primary() {
                    continue;
                }
                for id in [ALL_GROUP, vehicle.group_id] {
                    let group = groups
                        .get_mut(&(vehicle.owner, vehicle.kind, id))
                        .ok_or(VehicleCallbackError::State)?;
                    let profit = vehicle.profit_last_year >> 8;
                    group.profit_last_year = group.profit_last_year.saturating_add(profit);
                    if vehicle.economy_age > 730 {
                        group.profit_last_year_min_age =
                            group.profit_last_year_min_age.saturating_add(profit);
                        group.num_vehicle_min_age = group.num_vehicle_min_age.wrapping_add(1);
                    }
                }
            }
        }
    }
    Ok(state)
}
