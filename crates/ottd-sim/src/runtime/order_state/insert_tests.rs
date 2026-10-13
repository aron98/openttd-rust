use super::*;
use crate::runtime::purchase_tests::fixture::{fixture, request};
use crate::{CommandRequest, CommandReturn};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn insertion(vehicle: u32, index: u16) -> Result<CommandRequest> {
    Ok(serde_json::from_value(serde_json::json!({
        "company": 0, "mode": "post", "command": {
            "kind": "insert_order", "location": 0, "vehicle": vehicle, "index": index,
            "order": {"type": 2, "flags": 18, "dest": 0, "refit_cargo": 254,
                "wait_time": 0, "travel_time": 0, "max_speed": 65535}
        }
    }))?)
}

#[test]
fn insert_order_allocates_then_updates_selected_indices_without_rng() -> Result {
    // Given a purchased vehicle with no orders.
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("returns")?
        .result
    else {
        return Err("vehicle".into());
    };
    let rng = crate::runtime::purchase_tests::fixture::random(runtime.world())?;
    let initial_pool = runtime.order_list_pool();
    // When inserting two scheduled nearest-depot orders at the beginning.
    for _ in 0..2 {
        let receipt = runtime.execute_command(&insertion(vehicle, 0)?)?;
        assert!(receipt.posted);
        assert_eq!(receipt.result.ok_or("result")?.cost, 0);
    }
    // Then only one list is allocated, selected indices move and RNG stays unchanged.
    assert_eq!(runtime.order_list_pool().items, initial_pool.items + 1);
    let common = view::consist(runtime.world(), vehicle)?;
    assert_eq!(common.number("cur_real_order_index")?, 1);
    assert_eq!(common.number("cur_implicit_order_index")?, 1);
    assert_eq!(
        crate::runtime::purchase_tests::fixture::random(runtime.world())?,
        rng
    );
    let restored = SimulationRuntime::restore_vanilla(World::decode(&runtime.to_savegame()?)?)?;
    assert_eq!(
        restored.world().saved_json()?,
        runtime.world().saved_json()?
    );
    assert_eq!(restored.world().derived(), runtime.world().derived());
    Ok(())
}

#[test]
fn invalid_insert_index_and_estimate_do_not_publish_world_or_allocation() -> Result {
    // Given a purchased orderless vehicle.
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("returns")?
        .result
    else {
        return Err("vehicle".into());
    };
    let saved = runtime.world().saved_json()?;
    let derived = runtime.world().derived().clone();
    let orders = runtime.orders.clone();
    // When posting an out-of-range insertion or estimating a valid insertion.
    let rejected = runtime.execute_command(&insertion(vehicle, 1)?)?;
    assert!(!rejected.posted);
    let mut estimate = insertion(vehicle, 0)?;
    estimate.mode = crate::CommandMode::Estimate;
    let receipt = runtime.execute_command(&estimate)?;
    // Then neither world nor runtime allocator is changed.
    assert!(receipt.posted);
    assert!(receipt.exec.is_none());
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.orders, orders);
    Ok(())
}

#[test]
fn late_order_pool_mismatch_rolls_back_candidate_world_and_runtime() -> Result {
    // Given an otherwise valid command whose resident allocation metadata is stale.
    let (mut runtime, tile) = fixture()?;
    let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
        .execute_command(&request(tile))?
        .returns
        .ok_or("returns")?
        .result
    else {
        return Err("vehicle".into());
    };
    runtime.orders.lists.insert(63000)?;
    let saved = runtime.world().saved_json()?;
    let derived = runtime.world().derived().clone();
    let orders = runtime.orders.clone();
    let road = runtime.road.clone();
    // When publication reaches prepared-candidate allocation validation.
    let result = runtime.execute_command(&insertion(vehicle, 0)?);
    // Then none of the candidate ORDL/VEHS writes or runtime changes escape.
    assert!(matches!(
        result,
        Err(crate::CommandError::Runtime(RuntimeError::Invalid(
            "order list allocation membership"
        )))
    ));
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.orders, orders);
    assert_eq!(runtime.road, road);
    Ok(())
}

#[test]
fn absent_secondary_error_preserves_existing_receipt_encoding() -> Result {
    // Given a command with the existing no-secondary-error shape.
    let cost = crate::CommandCost::success(0, 255);
    // When it is serialized and deserialized.
    let json = serde_json::to_value(&cost)?;
    // Then old receipt JSON remains byte-shape compatible.
    assert_eq!(
        json,
        serde_json::json!({"success":true,"cost":0,"expenses":255,"error":null,"error_params":[]})
    );
    assert_eq!(serde_json::from_value::<crate::CommandCost>(json)?, cost);
    Ok(())
}

#[test]
fn station_insertion_matches_original_command_saved_world() -> Result {
    // Given actual original-command-built stops, depot and vehicles.
    let world = World::decode(&ottd_save::Savegame::decode(
        include_bytes!("../../../../../fixtures/road-orders/stations-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let expected = World::decode(&ottd_save::Savegame::decode(
        include_bytes!("../../../../../fixtures/road-orders/first-order-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let mut request = insertion(0, 0)?;
    let crate::Command::InsertOrder { order, .. } = &mut request.command else {
        return Err("order command".into());
    };
    order.kind = 33;
    order.flags = 0;
    // When posting the same station order through the owned runtime.
    let receipt = runtime.execute_command(&request)?;
    // Then every saved field and derived index matches the original checkpoint.
    assert!(receipt.posted);
    assert_eq!(runtime.world().saved_json()?, expected.saved_json()?);
    assert_eq!(runtime.world().derived(), expected.derived());
    Ok(())
}

#[test]
fn invalid_cargo_sentinel_is_rejected_as_an_order_refit() -> Result {
    // Given the native INVALID_CARGO value, distinct from CARGO_NO_REFIT.
    let world = World::decode(&ottd_save::Savegame::decode(
        include_bytes!("../../../../../fixtures/road-orders/stations-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let saved = runtime.world().saved_json()?;
    let mut request = insertion(0, 0)?;
    let crate::Command::InsertOrder { order, .. } = &mut request.command else {
        return Err("order command".into());
    };
    order.refit_cargo = 255;
    // When inserting a depot order carrying that refit selector.
    let receipt = runtime.execute_command(&request)?;
    // Then native validation rejects before allocating or changing saved state.
    assert!(!receipt.posted);
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("CMD_ERROR")
    );
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.order_list_pool().items, 0);
    Ok(())
}
