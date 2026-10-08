//! Vehicle callback boundary behavior independent of timer dispatch.
use ottd_sim::{VehicleCallbacks, VehicleOperation, run_vehicle_callback};

fn state() -> Result<VehicleCallbacks, serde_json::Error> {
    let mut state: VehicleCallbacks = serde_json::from_str(
        r#"{
      "human_companies":[0], "old_vehicle_warn":false,"vehicle_income_warn":false,
      "random_state":[305419896,87654321],
      "vehicles":[{"id":0,"owner":0,"kind":"train","subtype":9,"group_id":65534,
        "age":999,"max_age":1000,"economy_age":731,"reliability_spd_dec":32769,
        "profit_this_year":-1,"profit_last_year":512}],
      "groups":[
        {"owner":0,"kind":"train","id":65533,"profit_last_year":99,"profit_last_year_min_age":99,"num_vehicle_min_age":9,"num_vehicle":1},
        {"owner":0,"kind":"train","id":65534,"profit_last_year":99,"profit_last_year_min_age":99,"num_vehicle_min_age":9,"num_vehicle":1}]
    }"#,
    )?;
    for kind in [
        ottd_sim::VehicleKind::Road,
        ottd_sim::VehicleKind::Ship,
        ottd_sim::VehicleKind::Aircraft,
    ] {
        for id in [65533, 65534] {
            state.groups.push(ottd_sim::CallbackGroup {
                owner: 0,
                kind,
                id,
                profit_last_year: 99,
                profit_last_year_min_age: 99,
                num_vehicle_min_age: 9,
                num_vehicle: 1,
            });
        }
    }
    Ok(state)
}

#[test]
fn yearly_rollover_uses_signed_fixed_point_profit() -> Result<(), Box<dyn std::error::Error>> {
    // Given a human-owned primary engine, old enough to count, losing a fraction.
    let before = state()?;
    // When the yearly callback runs.
    let after = run_vehicle_callback(before, VehicleOperation::EconomyYear)?;
    // Then raw profits roll over and display profits floor toward negative infinity.
    let vehicle = after.vehicles.first().ok_or("vehicle missing")?;
    assert_eq!(
        (vehicle.profit_this_year, vehicle.profit_last_year),
        (0, -1)
    );
    for group in after
        .groups
        .iter()
        .filter(|g| g.kind == ottd_sim::VehicleKind::Train)
    {
        assert_eq!(
            (group.profit_last_year, group.profit_last_year_min_age),
            (-1, -1)
        );
        assert_eq!((group.num_vehicle, group.num_vehicle_min_age), (1, 1));
    }
    Ok(())
}

#[test]
fn calendar_anniversary_wraps_decay_without_changing_profits()
-> Result<(), Box<dyn std::error::Error>> {
    // Given a train immediately before its age limit.
    let before = state()?;
    // When its pool slot is scheduled.
    let after = run_vehicle_callback(before, VehicleOperation::CalendarDay { date_fract: 0 })?;
    // Then age increases and the native uint16 decay doubles with truncation.
    let vehicle = after.vehicles.first().ok_or("vehicle missing")?;
    assert_eq!((vehicle.age, vehicle.reliability_spd_dec), (1000, 2));
    assert_eq!(vehicle.profit_this_year, -1);
    Ok(())
}

#[test]
fn unsupported_warning_context_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    // Given a request requiring news support.
    let mut before = state()?;
    before.old_vehicle_warn = true;
    // When the callback is requested.
    let result = run_vehicle_callback(before, VehicleOperation::CalendarDay { date_fract: 0 });
    // Then the unsupported branch is rejected.
    assert!(result.is_err());
    Ok(())
}

#[test]
fn group_money_saturates_when_display_profits_overflow() -> Result<(), Box<dyn std::error::Error>> {
    // Given 300 primary vehicles whose display profit total exceeds signed money.
    let mut before = state()?;
    let template = before.vehicles.first().ok_or("vehicle missing")?.clone();
    before.vehicles = (0..300)
        .map(|id| {
            let mut vehicle = template.clone();
            vehicle.id = id;
            vehicle.profit_this_year = i64::MAX;
            vehicle
        })
        .collect();
    // When yearly group profits are rebuilt.
    let after = run_vehicle_callback(before, VehicleOperation::EconomyYear)?;
    // Then Money's native saturation is retained.
    assert!(
        after
            .groups
            .iter()
            .filter(|group| group.kind == ottd_sim::VehicleKind::Train)
            .all(|group| group.profit_last_year == i64::MAX)
    );
    Ok(())
}

#[test]
fn conflicting_global_group_id_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    // Given two ordinary groups claiming the same native pool slot.
    let mut before = state()?;
    let mut group = before.groups.first().ok_or("group missing")?.clone();
    group.id = 42;
    before.groups.push(group.clone());
    group.kind = ottd_sim::VehicleKind::Ship;
    before.groups.push(group);
    // When the yearly callback is requested.
    let result = run_vehicle_callback(before, VehicleOperation::EconomyYear);
    // Then impossible native pool identity is rejected.
    assert!(result.is_err());
    Ok(())
}

#[test]
fn missing_empty_company_bucket_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    // Given an omitted cache for a vehicle type with no vehicles.
    let mut before = state()?;
    before.groups.pop();
    // When yearly group profits would clear every native company bucket.
    let result = run_vehicle_callback(before, VehicleOperation::EconomyYear);
    // Then incomplete state is rejected rather than silently skipping the bucket.
    assert!(result.is_err());
    Ok(())
}

#[test]
fn native_vehicle_pool_end_is_exclusive() -> Result<(), Box<dyn std::error::Error>> {
    // Given a vehicle at the native exclusive pool limit.
    let mut before = state()?;
    before.vehicles.first_mut().ok_or("vehicle missing")?.id = 0xff000;
    // When its callback is requested.
    let result = run_vehicle_callback(before, VehicleOperation::EconomyYear);
    // Then that ID is rejected.
    assert!(result.is_err());
    Ok(())
}

#[test]
fn last_native_vehicle_pool_slot_is_valid() -> Result<(), Box<dyn std::error::Error>> {
    // Given the final native allocatable pool slot.
    let mut before = state()?;
    before.vehicles.first_mut().ok_or("vehicle missing")?.id = 0xfefff;
    // When its yearly callback is requested.
    let result = run_vehicle_callback(before, VehicleOperation::EconomyYear);
    // Then the pool boundary is accepted.
    assert!(result.is_ok());
    Ok(())
}
