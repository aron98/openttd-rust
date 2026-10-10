use super::*;

#[test]
fn invalid_sale_candidate_rolls_back_refund_and_freed_identity() -> Result {
    let (mut runtime, tile) = fixture()?;
    let first = bought(runtime.execute_command(&request(tile))?)?;
    let second = bought(runtime.execute_command(&request(tile))?)?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    let serializer = runtime.serializer_cargo_paid_for;
    let edits = vec![
        crate::world_access::field_edit(*b"PLYR", 0, "money", WireValue::Signed(123)),
        WorldEdit::RemoveRecord {
            chunk: *b"VEHS",
            record: first,
        },
        common(
            second,
            "next_shared",
            WireValue::Unsigned(u64::from(first).saturating_add(1)),
        ),
    ];
    let result = RoadVehicleContext {
        orders: &mut runtime.orders,
        serializer_cargo_paid_for: serializer,
        content: &runtime.content,
        allocation: &mut runtime.allocation,
        road: &mut runtime.road,
    }
    .publish_sale(&mut runtime.world, edits, VehicleId::new(first), None);
    assert!(
        matches!(result, Err(crate::CommandError::World(_))),
        "{result:?}"
    );
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    assert_eq!(runtime.serializer_cargo_paid_for, serializer);
    assert!(runtime.execute_command(&sale(tile, first))?.posted);
    assert_eq!(bought(runtime.execute_command(&request(tile))?)?, first);
    assert_eq!(
        SavedVehicleView::new(runtime.world(), VehicleId::new(first))?.unit_number()?,
        1
    );
    Ok(())
}

#[test]
fn refund_uses_native_saturating_negation_and_affordability() -> Result {
    for (value, money, expected) in [
        (77, 0, -77),
        (0, -1, 0),
        (-9, 20, 9),
        (i64::MIN, i64::MAX, i64::MAX),
    ] {
        let (mut runtime, tile) = fixture()?;
        let id = bought(runtime.execute_command(&request(tile))?)?;
        let mut world = runtime.into_world();
        world.edit_batch(vec![
            common(id, "value", WireValue::Signed(value)),
            crate::world_access::field_edit(*b"PLYR", 0, "money", WireValue::Signed(money)),
        ])?;
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        let receipt = runtime.execute_command(&sale(tile, id))?;
        assert_eq!(receipt.exec.ok_or("sale exec")?.cost, expected);
        assert_eq!(
            crate::world_access::signed(runtime.world(), b"PLYR", 0, "money")?,
            money.saturating_sub(expected)
        );
    }
    Ok(())
}
