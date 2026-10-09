use super::*;

#[test]
fn crashed_precedes_nonzero_speed_and_failure_preserves_runtime() -> Result {
    let (mut runtime, tile) = fixture()?;
    let id = bought(runtime.execute_command(&request(tile))?)?;
    let mut world = runtime.into_world();
    world.edit_batch(vec![
        common(id, "vehstatus", WireValue::Unsigned(128)),
        common(id, "cur_speed", WireValue::Unsigned(1)),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.world.saved_json()?;
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    let receipt = runtime.execute_command(&sale(tile, id))?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_VEHICLE_IS_DESTROYED")
    );
    assert!(receipt.exec.is_none());
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    Ok(())
}

#[test]
fn unsupported_order_backup_is_not_silently_discarded() -> Result {
    let (mut runtime, tile) = fixture()?;
    let id = bought(runtime.execute_command(&request(tile))?)?;
    let before = runtime.world.saved_json()?;
    let mut command = sale(tile, id);
    if let Command::SellVehicle { backup_order, .. } = &mut command.command {
        *backup_order = true;
    }
    assert!(matches!(
        runtime.execute_command(&command),
        Err(crate::CommandError::Unsupported("sale order backups"))
    ));
    assert_eq!(runtime.world.saved_json()?, before);
    Ok(())
}

#[test]
fn aged_zero_profit_vehicle_requires_full_statistics_lifecycle() -> Result {
    let (mut runtime, tile) = fixture()?;
    let id = bought(runtime.execute_command(&request(tile))?)?;
    let mut world = runtime.into_world();
    world.edit_batch(vec![common(id, "economy_age", WireValue::Signed(731))])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.world.saved_json()?;
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    assert!(matches!(
        runtime.execute_command(&sale(tile, id)),
        Err(crate::CommandError::Unsupported(
            "sale group profit or renewal lifecycle"
        ))
    ));
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    Ok(())
}
