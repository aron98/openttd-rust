use super::restore_tests::{build, copied_backup, sell};
use super::*;
use ottd_save::{
    TableRecord, WireValue,
    world::{PathElement, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn restore_poor_and_invalid_engine_leave_backups_rng_and_allocators_unchanged() -> Result {
    for poor in [false, true] {
        let (mut runtime, tile, _, _) = copied_backup()?;
        let mut request = build(tile, 77);
        if poor {
            runtime
                .world
                .edit_batch(vec![crate::world_access::field_edit(
                    *b"PLYR",
                    0,
                    "money",
                    WireValue::Signed(0),
                )])?;
        } else if let crate::Command::BuildVehicle { engine, .. } = &mut request.command {
            *engine = 65535;
        }
        let saved = runtime.world.saved_json()?;
        let derived = runtime.world.derived().clone();
        let orders = runtime.orders.clone();
        let allocation = runtime.allocation.clone();
        let road = runtime.road.clone();
        let result = runtime.execute_command(&request)?;
        assert!(!result.result.ok_or("result")?.success);
        assert!(result.exec.is_none());
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(runtime.world.derived(), &derived);
        assert_eq!(runtime.orders, orders);
        assert_eq!(runtime.allocation, allocation);
        assert_eq!(runtime.road, road);
    }
    Ok(())
}
#[test]
fn restore_rejects_matching_clone_group_and_live_non_sp_before_mutation() -> Result {
    for domain in 0..3 {
        let (mut runtime, tile, _, first, _) = super::tests::shared_fixture()?;
        runtime.backup_orders(VehicleId::new(first), 77)?;
        if domain == 1 {
            runtime.world.edit_batch(vec![
                crate::world_access::field_edit(*b"BKOR", 0, "clone", WireValue::Unsigned(0)),
                crate::world_access::field_edit(*b"BKOR", 0, "group", WireValue::Unsigned(65533)),
            ])?;
        } else if domain == 2 {
            runtime.orders.context = RuntimeSaveContext::NetworkServer;
        }
        let saved = runtime.world.saved_json()?;
        let orders = runtime.orders.clone();
        let allocation = runtime.allocation.clone();
        let road = runtime.road.clone();
        let result = runtime.execute_command(&build(tile, 77));
        assert!(matches!(
            result,
            Err(crate::CommandError::Runtime(RuntimeError::Unsupported(_)))
        ));
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(runtime.orders, orders);
        assert_eq!(runtime.allocation, allocation);
        assert_eq!(runtime.road, road);
    }
    Ok(())
}
#[test]
fn restore_preserves_nonempty_renewal_rows_and_company_link() -> Result {
    let (mut runtime, tile, _, _) = copied_backup()?;
    runtime.world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"ERNW",
            record: 0,
            value: TableRecord::new(vec![
                WireValue::Unsigned(116),
                WireValue::Unsigned(117),
                WireValue::Unsigned(0),
                WireValue::Unsigned(65534),
                WireValue::Signed(0),
            ]),
        },
        WorldEdit::Field {
            chunk: *b"PLYR",
            record: 0,
            path: vec![
                PathElement::Field("settings".into()),
                PathElement::Index(0),
                PathElement::Field("engine_renew_list".into()),
            ],
            value: WireValue::Unsigned(1),
        },
    ])?;
    let expected = view::table(runtime.world(), *b"ERNW")?.clone();
    assert!(runtime.execute_command(&build(tile, 77))?.posted);
    assert_eq!(view::table(runtime.world(), *b"ERNW")?, &expected);
    assert_eq!(
        runtime
            .world
            .saved_json()?
            .pointer("/chunks/PLYR/records/0/settings/0/engine_renew_list"),
        Some(&serde_json::json!(1))
    );
    Ok(())
}
fn common(id: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    }
}
#[test]
fn restore_cache_failure_rolls_back_consumption_list_allocation_money_and_rng() -> Result {
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    assert!(runtime.execute_command(&sell(tile, second, false))?.posted);
    runtime.backup_orders(VehicleId::new(first), 77)?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let mut candidate = allocation.clone();
    let id = candidate.pool.allocate()?;
    candidate.road_units.entry(0).or_default().use_id(2);
    let record = view::row(runtime.world(), *b"VEHS", first)?.record.clone();
    let edits = vec![
        WorldEdit::InsertRecord {
            chunk: *b"VEHS",
            record: id,
            value: record,
        },
        common(id, "orders", WireValue::Unsigned(0)),
        common(id, "next_shared", WireValue::Unsigned(0)),
        common(id, "engine_type", WireValue::Unsigned(0)),
        common(id, "unitnumber", WireValue::Unsigned(2)),
        crate::world_access::field_edit(*b"PLYR", 0, "money", WireValue::Signed(123)),
        crate::world_access::field_edit(*b"DATE", 0, "random_state[0]", WireValue::Unsigned(987)),
    ];
    let result = super::super::RoadVehicleContext {
        orders: &mut runtime.orders,
        serializer_cargo_paid_for: runtime.serializer_cargo_paid_for,
        content: &runtime.content,
        allocation: &mut runtime.allocation,
        road: &mut runtime.road,
    }
    .publish(&mut runtime.world, edits, candidate, VehicleId::new(id), 77);
    assert!(matches!(
        result,
        Err(crate::CommandError::Runtime(RuntimeError::Invalid(
            "road vehicle engine type"
        )))
    ));
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    Ok(())
}

#[test]
fn restore_copies_native_bits_and_skips_implicit_orders_without_copying_current_order() -> Result {
    let (mut runtime, tile, _, _) = copied_backup()?;
    let mut edits = Vec::new();
    for (name, value) in [
        ("name", WireValue::Bytes(b"Restored".to_vec())),
        ("current_order_time", WireValue::Unsigned(0x8000_0000)),
        ("lateness_counter", WireValue::Signed(-117)),
        ("timetable_start", WireValue::Unsigned(123)),
        ("vehicle_flags", WireValue::Unsigned(0x338)),
        ("service_interval", WireValue::Unsigned(75)),
        ("cur_real_order_index", WireValue::Unsigned(0)),
        ("cur_implicit_order_index", WireValue::Unsigned(250)),
    ] {
        edits.push(crate::world_access::field_edit(*b"BKOR", 0, name, value));
    }
    edits.push(WorldEdit::Field {
        chunk: *b"BKOR",
        record: 0,
        path: vec![
            PathElement::Field("orders".into()),
            PathElement::Index(0),
            PathElement::Field("type".into()),
        ],
        value: WireValue::Unsigned(8),
    });
    runtime.world.edit_batch(edits)?;
    let receipt = runtime.execute_command(&build(tile, 77))?;
    let Some(crate::CommandReturn::Vehicle { vehicle, .. }) =
        receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle".into());
    };
    let row = view::consist(runtime.world(), vehicle)?;
    assert_eq!(row.field("name")?, &WireValue::Bytes(b"Restored".to_vec()));
    assert_eq!(row.signed("current_order_time")?, i64::from(i32::MIN));
    assert_eq!(row.signed("lateness_counter")?, -117);
    assert_eq!(row.number("vehicle_flags")? & 0x338, 0x338);
    assert_eq!(row.number("cur_real_order_index")?, 1);
    assert_eq!(row.number("cur_implicit_order_index")?, 1);
    assert_eq!(row.number("current_order.type")?, 0);
    assert_eq!(row.signed("round_trip_time")?, 0);
    assert_eq!(
        World::decode(&runtime.to_savegame()?)?.saved_json()?,
        runtime.saved_json()?
    );
    Ok(())
}

#[test]
fn restore_full_list_pool_consumes_backup_and_properties_without_allocating() -> Result {
    // Saved-shaped orphan records supply the exact pool capacity, not a gameplay insertion claim.
    let (mut runtime, tile, _, _) = copied_backup()?;
    let resident = view::table(runtime.world(), *b"ORDL")?.records();
    let mut edits = (0..64000)
        .filter(|id| !resident.contains_key(id))
        .map(|id| WorldEdit::InsertRecord {
            chunk: *b"ORDL",
            record: id,
            value: TableRecord::new(vec![WireValue::Structs(Vec::new())]),
        })
        .collect::<Vec<_>>();
    edits.push(crate::world_access::field_edit(
        *b"BKOR",
        0,
        "current_order_time",
        WireValue::Unsigned(0x8000_0000),
    ));
    runtime.world.edit_batch(edits)?;
    runtime.orders.lists = PoolAllocator::restore(64000, 128, 0..64000)?;
    let before = runtime.order_list_pool();
    let receipt = runtime.execute_command(&build(tile, 77))?;
    let Some(crate::CommandReturn::Vehicle { vehicle, .. }) =
        receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle".into());
    };
    assert_eq!(runtime.order_list_pool(), before);
    assert_eq!(runtime.order_backup_pool().items, 0);
    assert_eq!(
        view::consist(runtime.world(), vehicle)?.number("orders")?,
        0
    );
    assert_eq!(
        view::consist(runtime.world(), vehicle)?.signed("current_order_time")?,
        i64::from(i32::MIN)
    );
    Ok(())
}
