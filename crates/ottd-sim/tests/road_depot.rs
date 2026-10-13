//! Road depot transactional command behavior.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{Command, CommandMode, CommandRequest, runtime::SimulationRuntime};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn fixture() -> Result<(SimulationRuntime, u32)> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"DATE",
        record: 0,
        path: vec![PathElement::Field("pause_mode".into())],
        value: WireValue::Unsigned(0),
    }])?;
    let tile = world
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find_map(|(i, t)| {
            let i = u32::try_from(i).ok()?;
            (i > world.map().width()
                && t.tile_type() >> 4 == 0
                && ottd_sim::terrain::tile_slope_z(&world, i).is_ok_and(|(s, _)| s.raw() == 0))
            .then_some(i)
        })
        .ok_or("flat clear tile")?;
    Ok((SimulationRuntime::restore_vanilla(world)?, tile))
}
const fn request(tile: u32, direction: u8) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::BuildRoadDepot {
            tile,
            road_type: 0,
            direction,
        },
    }
}

#[test]
fn nested_clear_error_keeps_accumulated_foundation_cost_and_expense() -> Result {
    let (mut runtime, tile) = fixture()?;
    assert!(runtime.execute_command(&request(tile, 0))?.posted);
    let mut world = runtime.into_world();
    let mut depot = ottd_save::TileRawParts::from(
        world
            .map()
            .tiles()
            .get(usize::try_from(tile)?)
            .ok_or("tile")?,
    );
    depot.m4 = 63;
    depot.m8 = 1 << 6;
    let east = tile.checked_add(world.map().width()).ok_or("east")?;
    let mut corner = ottd_save::TileRawParts::from(
        world
            .map()
            .tiles()
            .get(usize::try_from(east)?)
            .ok_or("east")?,
    );
    corner.height = corner.height.checked_add(1).ok_or("height")?;
    world.edit_batch(vec![
        WorldEdit::Tile {
            index: tile,
            value: depot.into(),
        },
        WorldEdit::Tile {
            index: east,
            value: corner.into(),
        },
        field(
            *b"PATS",
            0,
            "construction.build_on_slopes",
            WireValue::Signed(1),
        ),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let saved = runtime.world().saved_json()?;
    let pool = runtime.depot_pool();
    let foundation = runtime
        .content()
        .price(ottd_sim::content::Price::BuildFoundation);
    let result = runtime
        .execute_command(&request(tile, 0))?
        .result
        .ok_or("result")?;
    assert_eq!(
        result.error.as_deref(),
        Some("STR_ERROR_BUILDING_MUST_BE_DEMOLISHED")
    );
    assert_eq!((result.cost, result.expenses), (foundation, 0));
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.depot_pool(), pool);
    Ok(())
}

fn field(chunk: [u8; 4], record: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}

#[test]
fn occupied_rotation_uses_maximum_corner_and_noop_precedes_occupancy() -> Result {
    let (mut runtime, tile) = fixture()?;
    assert!(runtime.execute_command(&request(tile, 0))?.posted);
    let mut world = runtime.into_world();
    world.edit_batch(vec![field(
        *b"ENGN",
        116,
        "company_avail",
        WireValue::Unsigned(1),
    )])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let purchase = CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::BuildVehicle {
            tile,
            engine: 116,
            cargo: 255,
            use_free_vehicles: false,
            client_id: 0,
        },
    };
    assert!(runtime.execute_command(&purchase)?.posted);
    let id = *runtime.road_caches().keys().next().ok_or("road vehicle")?;
    let mut world = runtime.into_world();
    let east = tile.checked_add(world.map().width()).ok_or("corner")?;
    let mut corner = ottd_save::TileRawParts::from(
        world
            .map()
            .tiles()
            .get(usize::try_from(east)?)
            .ok_or("east")?,
    );
    corner.height = corner.height.checked_add(1).ok_or("height")?;
    let vehicle_z = i64::from(corner.height) * 8;
    world.edit_batch(vec![
        WorldEdit::Tile {
            index: east,
            value: corner.into(),
        },
        field(
            *b"PATS",
            0,
            "construction.build_on_slopes",
            WireValue::Signed(1),
        ),
        WorldEdit::Field {
            chunk: *b"VEHS",
            record: id.raw(),
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("common".into()),
                PathElement::Index(0),
                PathElement::Field("z_pos".into()),
            ],
            value: WireValue::Signed(vehicle_z),
        },
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.world().saved_json()?;
    let result = runtime.execute_command(&request(tile, 1))?;
    assert_eq!(
        result.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_ROAD_VEHICLE_IN_THE_WAY")
    );
    assert_eq!(runtime.world().saved_json()?, before);
    let noop = runtime.execute_command(&request(tile, 0))?;
    assert_eq!(noop.exec.ok_or("exec")?.cost, 0);
    Ok(())
}

#[test]
fn affordability_failure_preserves_pool_and_free_noop_accepts_negative_cash() -> Result {
    let (mut runtime, tile) = fixture()?;
    assert!(runtime.execute_command(&request(tile, 0))?.posted);
    let mut world = runtime.into_world();
    world.edit_batch(vec![field(*b"PLYR", 0, "money", WireValue::Signed(-1))])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.world().saved_json()?;
    let pool = runtime.depot_pool();
    let result = runtime.execute_command(&request(tile, 1))?;
    assert_eq!(
        result.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY")
    );
    assert_eq!(runtime.world().saved_json()?, before);
    assert_eq!(runtime.depot_pool(), pool);
    assert!(runtime.execute_command(&request(tile, 0))?.posted);
    Ok(())
}

#[test]
fn create_rotate_and_noop_preserve_depot_identity() -> Result {
    let (mut runtime, tile) = fixture()?;
    let pool = runtime.depot_pool();
    let counters = runtime.road_infrastructure()[&0];
    let created = runtime.execute_command(&request(tile, 0))?;
    assert!(created.exec.ok_or("exec")?.success);
    let records = runtime
        .world()
        .tables()
        .get(b"DEPT")
        .ok_or("DEPT")?
        .records()
        .clone();
    assert_eq!(runtime.depot_pool().items, pool.items + 1);
    assert_eq!(runtime.road_infrastructure()[&0][0], counters[0] + 2);
    let allocated = runtime.depot_pool();
    let rotated = runtime.execute_command(&request(tile, 1))?;
    assert!(rotated.exec.ok_or("exec")?.cost > 0);
    assert_eq!(runtime.depot_pool(), allocated);
    assert_eq!(
        runtime
            .world()
            .tables()
            .get(b"DEPT")
            .ok_or("DEPT")?
            .records(),
        &records
    );
    let noop = runtime.execute_command(&request(tile, 1))?;
    let cost = noop.exec.ok_or("exec")?;
    assert_eq!((cost.cost, cost.expenses), (0, 255));
    Ok(())
}

#[test]
fn estimate_and_invalid_direction_leave_world_and_runtime_unchanged() -> Result {
    let (mut runtime, tile) = fixture()?;
    let saved = runtime.world().saved_json()?;
    let pool = runtime.depot_pool();
    let counters = runtime.road_infrastructure().clone();
    let mut estimate = request(tile, 0);
    estimate.mode = CommandMode::Estimate;
    assert!(
        runtime
            .execute_command(&estimate)?
            .result
            .ok_or("result")?
            .success
    );
    let rejected = runtime.execute_command(&request(tile, 4))?;
    assert_eq!(
        rejected.result.ok_or("result")?.error.as_deref(),
        Some("CMD_ERROR")
    );
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.depot_pool(), pool);
    assert_eq!(runtime.road_infrastructure(), &counters);
    Ok(())
}
