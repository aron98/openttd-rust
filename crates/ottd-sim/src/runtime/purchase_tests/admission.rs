use super::*;

#[test]
fn zero_cost_purchase_executes_with_negative_cash() -> Result {
    let (runtime, tile) = fixture()?;
    let mut world = runtime.into_world();
    world.edit_batch(vec![
        crate::world_access::field_edit(*b"ECMY", 0, "inflation_prices", WireValue::Unsigned(0)),
        crate::world_access::field_edit(*b"PLYR", 0, "money", WireValue::Signed(-1)),
        crate::world_access::field_edit(
            *b"PATS",
            0,
            "difficulty.infinite_money",
            WireValue::Signed(0),
        ),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.world.saved_json()?;
    let mut estimate = request(tile);
    estimate.mode = CommandMode::Estimate;
    assert_eq!(
        runtime
            .execute_command(&estimate)?
            .result
            .ok_or("estimate")?
            .cost,
        0
    );
    assert_eq!(runtime.world.saved_json()?, before);
    let rng = random(runtime.world())?;
    let count = runtime.road.len();
    let receipt = runtime.execute_command(&request(tile))?;
    let executed = receipt.exec.ok_or("zero-cost purchase must execute")?;
    assert!(executed.success);
    assert_eq!(executed.cost, 0);
    assert_eq!(runtime.road.len(), count + 1);
    assert_eq!(
        crate::world_access::signed(runtime.world(), b"PLYR", 0, "money")?,
        -1
    );
    assert_ne!(random(runtime.world())?, rng);
    Ok(())
}

#[test]
fn missing_company_value_initializes_native_result_tuple() -> Result {
    let (mut runtime, tile) = fixture()?;
    let mut command = request(tile);
    command.company = 14;
    let before = runtime.world.saved_json()?;
    let receipt = runtime.execute_command(&command)?;
    let Some(crate::CommandReturn::Vehicle { vehicle, .. }) =
        receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle result".into());
    };
    assert_eq!(vehicle, 0);
    assert_eq!(runtime.world.saved_json()?, before);
    Ok(())
}

#[test]
fn refit_context_is_rejected_without_mutating_runtime() -> Result {
    let (mut runtime, tile) = fixture()?;
    let before = runtime.world.saved_json()?;
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    let mut command = request(tile);
    if let Command::BuildVehicle { cargo, .. } = &mut command.command {
        *cargo = 1;
    }
    assert!(matches!(
        runtime.execute_command(&command),
        Err(crate::CommandError::Unsupported(
            "vehicle refit construction"
        ))
    ));
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    Ok(())
}

#[test]
fn restoration_refuses_nonuniform_serialization_global() -> Result {
    let (mut runtime, tile) = fixture()?;
    assert!(runtime.execute_command(&request(tile))?.posted);
    assert!(runtime.execute_command(&request(tile))?.posted);
    let mut world = runtime.into_world();
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"VEHS",
        record: 0,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field("cargo_paid_for".into()),
        ],
        value: WireValue::Unsigned(37),
    }])?;
    assert!(matches!(
        SimulationRuntime::restore_vanilla(world),
        Err(RuntimeError::Unsupported(
            "nonuniform native cargo_paid_for serialization global"
        ))
    ));
    Ok(())
}

#[test]
fn estimate_preserves_every_runtime_object_and_obeys_company_limit() -> Result {
    let (mut runtime, tile) = fixture()?;
    let before = runtime.world.saved_json()?;
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    let mut req = request(tile);
    req.mode = CommandMode::Estimate;
    let receipt = runtime.execute_command(&req)?;
    assert!(receipt.posted);
    assert!(receipt.exec.is_none());
    let Some(crate::CommandReturn::Vehicle {
        vehicle, capacity, ..
    }) = receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle tuple".into());
    };
    assert_eq!((vehicle, capacity), (0xFFFFF, 31));
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    runtime
        .world
        .edit_batch(vec![crate::world_access::field_edit(
            *b"PATS",
            0,
            "vehicle.max_roadveh",
            WireValue::Unsigned(0),
        )])?;
    let before = runtime.world.saved_json()?;
    let rejected = runtime.execute_command(&req)?;
    assert_eq!(
        rejected.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_TOO_MANY_VEHICLES_IN_GAME")
    );
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    Ok(())
}

#[test]
fn insufficient_cash_preserves_test_tuple_and_all_mutable_state() -> Result {
    let (mut runtime, tile) = fixture()?;
    runtime
        .world
        .edit_batch(vec![crate::world_access::field_edit(
            *b"PLYR",
            0,
            "money",
            WireValue::Signed(0),
        )])?;
    let before = runtime.world.saved_json()?;
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    let receipt = runtime.execute_command(&request(tile))?;
    assert!(!receipt.posted);
    let test = receipt.test.ok_or("test")?;
    assert!(test.success && test.cost > 0);
    assert_eq!(test.expenses, 1);
    let result = receipt.result.ok_or("result")?;
    assert_eq!(
        result.error.as_deref(),
        Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY")
    );
    assert_eq!(result.cost, test.cost);
    let returns = receipt.returns.ok_or("returns")?;
    assert_eq!(returns.test, returns.result);
    assert!(returns.exec.is_none());
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    Ok(())
}

#[test]
fn wrong_depot_type_retains_native_purchase_cost_but_allocates_nothing() -> Result {
    let (mut runtime, tile) = fixture()?;
    let mut estimate = request(tile);
    estimate.mode = CommandMode::Estimate;
    let price = runtime
        .execute_command(&estimate)?
        .result
        .ok_or("cost")?
        .cost;
    let mut raw = TileRawParts::from(
        runtime
            .world
            .map()
            .tiles()
            .get(usize::try_from(tile)?)
            .ok_or("tile")?,
    );
    raw.m4 = 63;
    runtime.world.edit_batch(vec![WorldEdit::Tile {
        index: tile,
        value: raw.into(),
    }])?;
    let before = runtime.world.saved_json()?;
    let allocation = runtime.allocation.clone();
    let receipt = runtime.execute_command(&request(tile))?;
    let result = receipt.result.ok_or("cost")?;
    assert_eq!(
        result.error.as_deref(),
        Some("STR_ERROR_DEPOT_WRONG_DEPOT_TYPE")
    );
    assert_eq!((result.cost, result.expenses), (price, 1));
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    Ok(())
}

#[test]
fn nonroad_depot_is_explicit_unsupported_instead_of_a_fake_native_error() -> Result {
    let (mut runtime, _) = fixture()?;
    let (index, tile) = runtime
        .world
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 1 && t.m5() >> 6 == 3)
        .ok_or("rail depot")?;
    let index = u32::try_from(index)?;
    let mut raw = TileRawParts::from(tile);
    raw.m1 = 0;
    runtime.world.edit_batch(vec![WorldEdit::Tile {
        index,
        value: raw.into(),
    }])?;
    let before = runtime.world.saved_json()?;
    assert!(matches!(
        runtime.execute_command(&request(index)),
        Err(crate::CommandError::Unsupported(
            "non-road depot or airport construction"
        ))
    ));
    assert_eq!(runtime.world.saved_json()?, before);
    Ok(())
}
