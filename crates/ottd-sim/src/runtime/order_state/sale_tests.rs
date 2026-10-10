use super::*;
use crate::runtime::{RoadVehicleContext, purchase_tests::fixture::request};
use crate::{Command, CommandMode, CommandRequest, CommandReturn};
use ottd_save::{WireValue, world::PathElement};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn sale(tile: u32, vehicle: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::SellVehicle {
            location: tile,
            vehicle,
            sell_chain: false,
            backup_order: false,
            client_id: 77,
        },
    }
}
fn field(vehicle: u32, name: &str, value: WireValue) -> WorldEdit {
    use PathElement::{Field, Index};
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: vehicle,
        path: vec![
            Field("roadveh".into()),
            Index(0),
            Field("common".into()),
            Index(0),
            Field(name.into()),
        ],
        value,
    }
}
fn three() -> Result<(SimulationRuntime, u32, [u32; 3])> {
    let (mut runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    let Some(CommandReturn::Vehicle { vehicle: third, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("returns")?
        .result
    else {
        return Err("vehicle".into());
    };
    let mut world = runtime.into_world();
    world.edit_batch(vec![
        field(third, "orders", WireValue::Unsigned(60001)),
        field(
            second,
            "next_shared",
            WireValue::Unsigned(u64::from(third) + 1),
        ),
        field(
            second,
            "depot_unbunching_last_departure",
            WireValue::Unsigned(123),
        ),
        field(
            second,
            "depot_unbunching_next_departure",
            WireValue::Unsigned(456),
        ),
        field(second, "round_trip_time", WireValue::Signed(789)),
    ])?;
    Ok((
        SimulationRuntime::restore_vanilla(world)?,
        tile,
        [first, second, third],
    ))
}
#[test]
fn shared_head_middle_tail_keep_payload_timing_and_unrelated_lists() -> Result {
    for position in 0..3 {
        let (mut runtime, tile, ids) = three()?;
        let id = *ids.get(position).ok_or("position")?;
        let before = runtime.world.clone();
        let pool = runtime.order_list_pool();
        let expected: Vec<_> = ids.iter().copied().filter(|other| *other != id).collect();
        assert!(runtime.execute_command(&sale(tile, id))?.posted);
        assert_eq!(runtime.order_list_pool(), pool);
        assert_eq!(
            runtime.world.tables().get(b"ORDL"),
            before.tables().get(b"ORDL")
        );
        let list = runtime
            .world
            .derived()
            .order_lists
            .iter()
            .find(|list| list.id == 60000)
            .ok_or("list")?;
        assert_eq!(list.vehicles, expected);
        assert_eq!(list.first_shared, expected.first().copied());
        let old = before
            .derived()
            .order_lists
            .iter()
            .find(|list| list.id == 60000)
            .ok_or("before")?;
        assert_eq!(
            (
                list.num_manual_orders,
                list.total_duration,
                list.timetable_duration
            ),
            (
                old.num_manual_orders,
                old.total_duration,
                old.timetable_duration
            )
        );
        for survivor in expected {
            for name in [
                "depot_unbunching_last_departure",
                "depot_unbunching_next_departure",
                "round_trip_time",
            ] {
                let read = |world: &World| -> Result<WireValue> {
                    Ok(
                        crate::runtime::SavedVehicleView::new(world, VehicleId::new(survivor))?
                            .common_field(name)?
                            .clone(),
                    )
                };
                assert_eq!(read(&runtime.world)?, read(&before)?);
            }
        }
    }
    Ok(())
}
#[test]
fn ordered_estimate_and_failed_prepare_leave_all_order_lifetimes_unchanged() -> Result {
    let (mut runtime, tile, ids) = three()?;
    let first = *ids.first().ok_or("first")?;
    let saved = runtime.world.saved_json()?;
    let derived = runtime.world.derived().clone();
    let orders = runtime.orders.clone();
    let allocation = runtime.allocation.clone();
    let road = runtime.road.clone();
    let mut estimate = sale(tile, first);
    estimate.mode = CommandMode::Estimate;
    assert!(
        runtime
            .execute_command(&estimate)?
            .result
            .ok_or("result")?
            .success
    );
    let result = RoadVehicleContext {
        orders: &mut runtime.orders,
        serializer_cargo_paid_for: runtime.serializer_cargo_paid_for,
        content: &runtime.content,
        allocation: &mut runtime.allocation,
        road: &mut runtime.road,
    }
    .publish_sale(
        &mut runtime.world,
        vec![
            WorldEdit::RemoveRecord {
                chunk: *b"VEHS",
                record: first,
            },
            field(
                *ids.last().ok_or("tail")?,
                "next_shared",
                WireValue::Unsigned(u64::from(first) + 1),
            ),
        ],
        VehicleId::new(first),
    );
    assert!(result.is_err());
    assert_eq!(runtime.world.saved_json()?, saved);
    assert_eq!(runtime.world.derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, road);
    Ok(())
}
#[test]
fn ordered_sale_loading_and_live_backup_stay_explicit_boundaries() -> Result {
    for loading in [false, true] {
        let (runtime, tile, _, first, _) = super::tests::shared_fixture()?;
        let mut world = runtime.into_world();
        if loading {
            world.edit_batch(vec![field(
                first,
                "current_order.type",
                WireValue::Unsigned(3),
            )])?;
        }
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        if !loading {
            runtime.backup_orders(VehicleId::new(first), 77)?;
            (runtime, _) = SimulationRuntime::from_loaded_vanilla(
                runtime.into_world(),
                RuntimeSaveContext::NetworkClient,
            )?;
        }
        let before = runtime.world.saved_json()?;
        let orders = runtime.orders.clone();
        let result = runtime.execute_command(&sale(tile, first));
        assert!(matches!(result, Err(crate::CommandError::Unsupported(_))));
        assert_eq!(runtime.world.saved_json()?, before);
        assert_eq!(runtime.orders, orders);
    }
    Ok(())
}

#[test]
fn sole_order_pool_free_reuses_lowest_identity_without_clean_pool() -> Result {
    let (runtime, tile, _, first, second) = super::tests::shared_fixture()?;
    let mut available = runtime.orders.lists.clone();
    let low = available.allocate()?;
    let mut world = runtime.into_world();
    let row = world
        .tables()
        .get(b"ORDL")
        .ok_or("ORDL")?
        .records()
        .get(&60000)
        .ok_or("list")?
        .clone();
    world.edit_batch(vec![
        WorldEdit::RemoveRecord {
            chunk: *b"ORDL",
            record: 60000,
        },
        WorldEdit::InsertRecord {
            chunk: *b"ORDL",
            record: low,
            value: row,
        },
        field(first, "orders", WireValue::Unsigned(u64::from(low) + 1)),
        field(second, "orders", WireValue::Unsigned(u64::from(low) + 1)),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.order_list_pool();
    assert!(runtime.execute_command(&sale(tile, first))?.posted);
    assert!(runtime.execute_command(&sale(tile, second))?.posted);
    let after = runtime.order_list_pool();
    assert_eq!(after.slots, before.slots);
    assert_eq!(after.first_unused, before.first_unused);
    // This is allocator evidence; no order-creation command is admitted by this test.
    let mut next = runtime.orders.lists.clone();
    assert_eq!(next.allocate()?, low);
    Ok(())
}
