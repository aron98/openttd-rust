use super::restore_tests::build;
use super::*;
use crate::{CommandMode, CommandReturn};
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
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
fn returned(runtime: &mut SimulationRuntime, tile: u32) -> Result<u32> {
    let receipt = runtime.execute_command(&build(tile, 77))?;
    assert!(receipt.posted);
    let Some(CommandReturn::Vehicle { vehicle, .. }) = receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle".into());
    };
    Ok(vehicle)
}
#[test]
fn shared_restore_copies_properties_uses_live_orders_and_consumes_once() -> Result {
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 77)?;
    runtime.world.edit_batch(vec![
        crate::world_access::field_edit(
            *b"BKOR",
            0,
            "name",
            WireValue::Bytes(b"Shared restoration".to_vec()),
        ),
        crate::world_access::field_edit(
            *b"BKOR",
            0,
            "cur_real_order_index",
            WireValue::Unsigned(1),
        ),
        crate::world_access::field_edit(
            *b"BKOR",
            0,
            "cur_implicit_order_index",
            WireValue::Unsigned(250),
        ),
        crate::world_access::field_edit(
            *b"BKOR",
            0,
            "current_order_time",
            WireValue::Unsigned(0x8000_0000),
        ),
        crate::world_access::field_edit(*b"BKOR", 0, "vehicle_flags", WireValue::Unsigned(0x338)),
    ])?;
    let raw = view::table(runtime.world(), *b"ORDL")?.clone();
    let pool = runtime.order_list_pool();
    let id = returned(&mut runtime, tile)?;
    assert_eq!(view::table(runtime.world(), *b"ORDL")?, &raw);
    assert_eq!(runtime.order_list_pool(), pool);
    let row = view::consist(runtime.world(), id)?;
    assert_eq!(row.number("cur_real_order_index")?, 1);
    assert_eq!(row.number("cur_implicit_order_index")?, 1);
    assert_eq!(row.signed("current_order_time")?, i64::from(i32::MIN));
    assert_eq!(
        row.field("name")?,
        &WireValue::Bytes(b"Shared restoration".to_vec())
    );
    assert_eq!(row.number("vehicle_flags")? & 0x338, 0x338);
    assert_eq!(row.signed("round_trip_time")?, 0);
    assert_eq!(
        runtime
            .world
            .derived()
            .order_lists
            .iter()
            .find(|list| list.id == 60000)
            .ok_or("list")?
            .vehicles,
        [first, second, id]
    );
    let again = returned(&mut runtime, tile)?;
    assert_eq!(view::consist(runtime.world(), again)?.number("orders")?, 0);
    assert_eq!(runtime.order_backup_pool().items, 0);
    Ok(())
}
#[test]
fn shared_restore_estimate_and_failure_preserve_all_state() -> Result {
    for mode in 0..3 {
        let (mut runtime, tile, _, first, _) = super::tests::shared_fixture()?;
        runtime.backup_orders(VehicleId::new(first), 77)?;
        let mut request = build(tile, 77);
        if mode == 0 {
            request.mode = CommandMode::Estimate;
        }
        if mode == 1 {
            runtime
                .world
                .edit_batch(vec![crate::world_access::field_edit(
                    *b"PLYR",
                    0,
                    "money",
                    WireValue::Signed(0),
                )])?;
        }
        if mode == 2 {
            if let crate::Command::BuildVehicle { engine, .. } = &mut request.command {
                *engine = 65535;
            }
        }
        let saved = runtime.world.saved_json()?;
        let derived = runtime.world.derived().clone();
        let orders = runtime.orders.clone();
        let allocation = runtime.allocation.clone();
        let road = runtime.road.clone();
        let receipt = runtime.execute_command(&request)?;
        assert!(receipt.exec.is_none());
        assert_eq!(receipt.result.ok_or("result")?.success, mode == 0);
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(runtime.world.derived(), &derived);
        assert_eq!(runtime.orders, orders);
        assert_eq!(runtime.allocation, allocation);
        assert_eq!(runtime.road, road);
    }
    Ok(())
}
#[test]
fn shared_restore_rejects_unproved_domains_without_publication() -> Result {
    for domain in 0..5 {
        let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
        let unrelated = returned(&mut runtime, tile)?;
        runtime.backup_orders(VehicleId::new(first), 77)?;
        let edit = match domain {
            0 => common(second, "cargo_type", WireValue::Unsigned(1)),
            1 => common(unrelated, "owner", WireValue::Unsigned(1)),
            2 => crate::world_access::field_edit(
                *b"BKOR",
                0,
                "clone",
                WireValue::Unsigned(u64::from(unrelated) + 1),
            ),
            3 => WorldEdit::Field {
                chunk: *b"ORDL",
                record: 60000,
                path: vec![
                    PathElement::Field("orders".into()),
                    PathElement::Index(0),
                    PathElement::Field("type".into()),
                ],
                value: WireValue::Unsigned(1),
            },
            _ => crate::world_access::field_edit(*b"BKOR", 0, "group", WireValue::Unsigned(65533)),
        };
        let mut edits = vec![edit];
        if domain == 1 {
            edits.push(crate::world_access::field_edit(
                *b"BKOR",
                0,
                "clone",
                WireValue::Unsigned(u64::from(unrelated) + 1),
            ));
        }
        runtime.world.edit_batch(edits)?;
        let saved = runtime.world.saved_json()?;
        let derived = runtime.world.derived().clone();
        let orders = runtime.orders.clone();
        let allocation = runtime.allocation.clone();
        let road = runtime.road.clone();
        assert!(matches!(
            runtime.execute_command(&build(tile, 77)),
            Err(crate::CommandError::Runtime(RuntimeError::Unsupported(_)))
        ));
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(runtime.world.derived(), &derived);
        assert_eq!(runtime.orders, orders);
        assert_eq!(runtime.allocation, allocation);
        assert_eq!(runtime.road, road);
    }
    Ok(())
}
#[test]
fn shared_restore_candidate_failure_rolls_back_links_and_consumption() -> Result {
    let (mut runtime, tile, _, first, _) = super::tests::shared_fixture()?;
    runtime.backup_orders(VehicleId::new(first), 77)?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let mut candidate = allocation.clone();
    let id = candidate.pool.allocate()?;
    candidate.road_units.entry(0).or_default().use_id(3);
    let record = view::row(runtime.world(), *b"VEHS", first)?.record.clone();
    let edits = vec![
        WorldEdit::InsertRecord {
            chunk: *b"VEHS",
            record: id,
            value: record,
        },
        common(id, "orders", WireValue::Unsigned(0)),
        common(id, "next_shared", WireValue::Unsigned(0)),
        common(id, "unitnumber", WireValue::Unsigned(3)),
        common(id, "engine_type", WireValue::Unsigned(0)),
        crate::world_access::field_edit(*b"PLYR", 0, "money", WireValue::Signed(123)),
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
    let _ = tile;
    Ok(())
}
