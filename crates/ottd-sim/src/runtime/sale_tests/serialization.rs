use super::*;

#[test]
fn last_sale_preserves_serializer_for_live_repurchase() -> Result {
    let (runtime, tile) = fixture()?;
    let mut world = runtime.into_world();
    let setup = world
        .tables()
        .get(b"VEHS")
        .ok_or("VEHS")?
        .records()
        .keys()
        .map(|id| WorldEdit::RemoveRecord {
            chunk: *b"VEHS",
            record: *id,
        })
        .collect();
    world.edit_batch(setup)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let id = bought(runtime.execute_command(&request(tile))?)?;
    let mut world = runtime.into_world();
    world.edit_batch(vec![common(id, "cargo_paid_for", WireValue::Unsigned(37))])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let rng = random(runtime.world())?;
    assert!(runtime.execute_command(&sale(tile, id))?.posted);
    assert!(runtime.road.is_empty());
    assert_eq!(runtime.serializer_cargo_paid_for, 37);
    assert_eq!(random(runtime.world())?, rng);
    let new_id = bought(runtime.execute_command(&request(tile))?)?;
    assert_eq!(new_id, id);
    assert_eq!(
        SavedVehicleView::new(runtime.world(), VehicleId::new(new_id))?.cargo_paid_for()?,
        37
    );
    Ok(())
}
