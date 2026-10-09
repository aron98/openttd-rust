use super::*;
use crate::{Command, CommandMode, CommandRequest};
use ottd_save::{
    Savegame, TileRawParts, WireValue,
    world::{PathElement, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn fixture() -> Result<(SimulationRuntime, u32)> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let ids = world
        .tables()
        .get(b"VEHS")
        .ok_or("VEHS")?
        .records()
        .keys()
        .copied();
    let edits = ids
        .filter(|id| SavedVehicleView::new(&world, VehicleId::new(*id)).is_err())
        .map(|id| WorldEdit::RemoveRecord {
            chunk: *b"VEHS",
            record: id,
        })
        .collect();
    world.edit_batch(edits)?;
    let (index, tile) = world
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
        .ok_or("depot")?;
    let index = u32::try_from(index)?;
    let mut raw = TileRawParts::from(tile);
    raw.m1 = 0;
    world.edit_batch(vec![
        WorldEdit::Tile {
            index,
            value: raw.into(),
        },
        WorldEdit::Field {
            chunk: *b"ENGN",
            record: 116,
            path: vec![PathElement::Field("company_avail".into())],
            value: WireValue::Unsigned(1),
        },
        WorldEdit::Field {
            chunk: *b"DATE",
            record: 0,
            path: vec![PathElement::Field("pause_mode".into())],
            value: WireValue::Unsigned(0),
        },
    ])?;
    Ok((SimulationRuntime::restore_vanilla(world)?, index))
}
fn request(tile: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::BuildVehicle {
            tile,
            engine: 116,
            cargo: 255,
            use_free_vehicles: false,
            client_id: 0,
        },
    }
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
fn purchase_retains_native_serialization_global() -> Result {
    let (mut runtime, tile) = fixture()?;
    assert!(runtime.execute_command(&request(tile))?.posted);
    let mut world = runtime.into_world();
    let edits = world
        .tables()
        .get(b"VEHS")
        .ok_or("VEHS")?
        .records()
        .keys()
        .map(|id| WorldEdit::Field {
            chunk: *b"VEHS",
            record: *id,
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("common".into()),
                PathElement::Index(0),
                PathElement::Field("cargo_paid_for".into()),
            ],
            value: WireValue::Unsigned(37),
        })
        .collect();
    world.edit_batch(edits)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let receipt = runtime.execute_command(&request(tile))?;
    let Some(crate::CommandReturn::Vehicle { vehicle, .. }) =
        receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle result".into());
    };
    assert_eq!(
        runtime.world.saved_json()?.pointer(&format!(
            "/chunks/VEHS/records/{vehicle}/roadveh/0/common/0/cargo_paid_for"
        )),
        Some(&serde_json::json!(37))
    );
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
fn random(world: &World) -> Result<[u32; 2]> {
    Ok([
        u32::try_from(crate::world_access::unsigned(
            world,
            b"DATE",
            0,
            "random_state[0]",
        )?)?,
        u32::try_from(crate::world_access::unsigned(
            world,
            b"DATE",
            0,
            "random_state[1]",
        )?)?,
    ])
}
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
fn depot_direction_height_and_dynamic_engine_fields_drive_new_saved_state() -> Result {
    for direction in 0..4 {
        let (mut runtime, tile) = fixture()?;
        let tile_index = usize::try_from(tile)?;
        let mut raw =
            TileRawParts::from(runtime.world.map().tiles().get(tile_index).ok_or("tile")?);
        raw.m5 = (raw.m5 & !3) | direction;
        runtime.world.edit_batch(vec![
            WorldEdit::Tile {
                index: tile,
                value: raw.into(),
            },
            crate::world_access::field_edit(
                *b"ENGN",
                116,
                "reliability",
                WireValue::Unsigned(1234),
            ),
            crate::world_access::field_edit(
                *b"ENGN",
                116,
                "reliability_spd_dec",
                WireValue::Unsigned(17),
            ),
            crate::world_access::field_edit(*b"ENGN", 116, "flags", WireValue::Unsigned(2)),
            crate::world_access::field_edit(
                *b"PATS",
                0,
                "vehicle.extend_vehicle_life",
                WireValue::Unsigned(5),
            ),
        ])?;
        let calendar = crate::world_access::signed(&runtime.world, b"DATE", 0, "date")?;
        let economy = crate::world_access::signed(&runtime.world, b"DATE", 0, "economy_date")?;
        let receipt = runtime.execute_command(&request(tile))?;
        assert!(receipt.posted);
        let json = runtime.world.saved_json()?;
        let common = json
            .pointer("/chunks/VEHS/records/0/roadveh/0/common/0")
            .ok_or("common")?;
        assert_eq!(
            common.get("direction"),
            Some(&serde_json::json!(
                direction.saturating_mul(2).saturating_add(1)
            ))
        );
        assert_eq!(common.get("reliability"), Some(&serde_json::json!(1234)));
        assert_eq!(
            common.get("reliability_spd_dec"),
            Some(&serde_json::json!(17))
        );
        assert_eq!(
            common.get("max_age"),
            Some(&serde_json::json!((12 + 5) * 366))
        );
        assert_eq!(
            common.get("date_of_last_service"),
            Some(&serde_json::json!(economy))
        );
        assert_eq!(
            common.get("date_of_last_service_newgrf"),
            Some(&serde_json::json!(calendar))
        );
        assert_eq!(
            common
                .get("vehicle_flags")
                .and_then(serde_json::Value::as_u64)
                .ok_or("flags")?
                & 4,
            4
        );
        let width = runtime.world.map().width();
        let mut max_height = 0;
        for index in [
            tile,
            tile.saturating_add(1),
            tile.saturating_add(width),
            tile.saturating_add(width).saturating_add(1),
        ] {
            max_height = max_height.max(
                runtime
                    .world
                    .map()
                    .tiles()
                    .get(usize::try_from(index)?)
                    .ok_or("corner")?
                    .height(),
            );
        }
        assert_eq!(
            common.get("z_pos"),
            Some(&serde_json::json!(u16::from(max_height) * 8))
        );
    }
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
