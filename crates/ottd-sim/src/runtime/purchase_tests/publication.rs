use super::*;

#[test]
fn purchase_publishes_native_identity_rng_record_and_creation_cache() -> Result {
    let (mut runtime, tile) = fixture()?;
    let before = random(runtime.world())?;
    let mut rng = ottd_core::Randomizer::from_state(before);
    let bits = rng.next_u32();
    let receipt = runtime.execute_command(&request(tile))?;
    assert!(receipt.posted);
    let Some(crate::CommandReturn::Vehicle {
        vehicle,
        capacity,
        mail_capacity,
        ..
    }) = receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle tuple".into());
    };
    assert_eq!(vehicle, 0);
    assert_eq!(capacity, 31);
    assert_eq!(mail_capacity, 0);
    assert_eq!(random(runtime.world())?, rng.state());
    let json = runtime.world().saved_json()?;
    assert_eq!(
        json.pointer("/chunks/VEHS/records/0/roadveh/0/common/0/random_bits"),
        Some(&serde_json::json!(bits & 65535))
    );
    assert_eq!(runtime.road_cache(VehicleId::new(vehicle))?.last_speed, 0);
    assert_eq!(
        runtime.road_cache(VehicleId::new(vehicle))?.trip_occupancy,
        0
    );
    Ok(())
}

#[test]
fn actual_candidate_cache_failure_rolls_back_world_rng_units_and_pool() -> Result {
    let (mut runtime, tile) = fixture()?;
    runtime.execute_command(&request(tile))?;
    let before = runtime.world.saved_json()?;
    let before_derived = serde_json::to_value(runtime.world.derived())?;
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    let mut candidate = allocation.clone();
    let id = candidate.pool.allocate()?;
    candidate.road_units.entry(0).or_default().use_id(2);
    let record = runtime
        .world
        .tables()
        .get(b"VEHS")
        .and_then(|t| t.records().get(&0))
        .ok_or("built record")?
        .clone();
    let edits = vec![
        WorldEdit::InsertRecord {
            chunk: *b"VEHS",
            record: id,
            value: record,
        },
        WorldEdit::Field {
            chunk: *b"VEHS",
            record: id,
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("common".into()),
                PathElement::Index(0),
                PathElement::Field("engine_type".into()),
            ],
            value: WireValue::Unsigned(0),
        },
        crate::world_access::field_edit(*b"PLYR", 0, "money", WireValue::Signed(123)),
        crate::world_access::field_edit(*b"DATE", 0, "random_state[0]", WireValue::Unsigned(987)),
    ];
    let result = PurchaseContext {
        serializer_cargo_paid_for: runtime.serializer_cargo_paid_for,
        content: &runtime.content,
        allocation: &mut runtime.allocation,
        road: &mut runtime.road,
    }
    .publish(&mut runtime.world, edits, candidate, VehicleId::new(id));
    assert!(matches!(
        result,
        Err(crate::CommandError::Runtime(RuntimeError::Invalid(
            "road vehicle engine type"
        )))
    ));
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(
        serde_json::to_value(runtime.world.derived())?,
        before_derived
    );
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    let receipt = runtime.execute_command(&request(tile))?;
    let Some(crate::CommandReturn::Vehicle { vehicle, .. }) =
        receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle tuple".into());
    };
    assert_eq!(vehicle, id);
    assert_eq!(runtime.vehicle(VehicleId::new(id))?.unit_number()?, 2);
    Ok(())
}
