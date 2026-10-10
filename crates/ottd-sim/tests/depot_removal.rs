//! Native single-tile depot lifecycle through the owned command runtime.
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
        .find_map(|(index, tile)| {
            let index = u32::try_from(index).ok()?;
            (index > world.map().width()
                && tile.tile_type() >> 4 == 0
                && ottd_sim::terrain::tile_slope_z(&world, index)
                    .is_ok_and(|(slope, _)| slope.raw() == 0))
            .then_some(index)
        })
        .ok_or("flat clear tile")?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    assert!(runtime.execute_command(&build(tile))?.posted);
    Ok((runtime, tile))
}
const fn build(tile: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::BuildRoadDepot {
            tile,
            road_type: 0,
            direction: 0,
        },
    }
}
const fn clear(tile: u32, mode: CommandMode) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode,
        command: Command::LandscapeClear { tile },
    }
}
#[test]
fn depot_removal_publishes_map_and_allocator_once_then_reuses_id() -> Result {
    let (mut runtime, tile) = fixture()?;
    let before = runtime.depot_pool();
    let result = runtime.execute_command(&clear(tile, CommandMode::Post))?;
    assert!(result.posted);
    assert!(result.exec.is_some());
    assert!(
        runtime
            .world()
            .tables()
            .get(b"DEPT")
            .ok_or("DEPT")?
            .records()
            .is_empty()
    );
    assert_eq!(
        runtime
            .world()
            .map()
            .tiles()
            .get(usize::try_from(tile)?)
            .ok_or("tile")?
            .tile_type()
            >> 4,
        0
    );
    let freed = runtime.depot_pool();
    assert_eq!(freed.items, 0);
    assert_eq!(freed.first_unused, before.first_unused);
    assert_eq!(freed.slots, before.slots);
    assert!(runtime.execute_command(&build(tile))?.posted);
    assert_eq!(runtime.depot_pool().occupied, before.occupied);
    Ok(())
}
#[test]
fn depot_estimate_retains_complete_saved_state_and_live_pools() -> Result {
    let (mut runtime, tile) = fixture()?;
    let saved = runtime.world().saved_json()?;
    let depots = runtime.depot_pool();
    let backups = runtime.order_backup_pool();
    let receipt = runtime.execute_command(&clear(tile, CommandMode::Estimate))?;
    assert!(receipt.result.ok_or("result")?.success);
    assert!(receipt.exec.is_none());
    assert_eq!(runtime.world().saved_json()?, saved);
    assert_eq!(runtime.depot_pool(), depots);
    assert_eq!(runtime.order_backup_pool(), backups);
    Ok(())
}

#[test]
fn exhausted_clear_limit_fails_without_any_publication() -> Result {
    let (runtime, tile) = fixture()?;
    let mut world = World::decode(&runtime.to_savegame()?)?;
    world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"PLYR",
        record: 0,
        path: vec![PathElement::Field("clear_limit".into())],
        value: WireValue::Unsigned(65535),
    }])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.world().saved_json()?;
    let pool = runtime.depot_pool();
    let result = runtime.execute_command(&clear(tile, CommandMode::Post))?;
    assert!(!result.posted);
    assert_eq!(
        result.result.ok_or("result")?.error.as_deref(),
        Some("STR_ERROR_CLEARING_LIMIT_REACHED")
    );
    assert_eq!(runtime.world().saved_json()?, before);
    assert_eq!(runtime.depot_pool(), pool);
    Ok(())
}

#[test]
fn unsupported_network_host_does_not_mutate_depot() -> Result {
    let (runtime, tile) = fixture()?;
    for context in [
        ottd_sim::runtime::RuntimeSaveContext::NetworkClient,
        ottd_sim::runtime::RuntimeSaveContext::NetworkServer,
    ] {
        let world = World::decode(&runtime.to_savegame()?)?;
        let (mut runtime, _) = SimulationRuntime::from_loaded_vanilla(world, context)?;
        let before = runtime.world().saved_json()?;
        let pool = runtime.depot_pool();
        assert!(matches!(
            runtime.execute_command(&clear(tile, CommandMode::Post)),
            Err(ottd_sim::CommandError::Unsupported(
                "depot removal host context"
            ))
        ));
        assert_eq!(runtime.world().saved_json()?, before);
        assert_eq!(runtime.depot_pool(), pool);
    }
    Ok(())
}

#[test]
fn insufficient_and_exact_money_follow_test_then_execution() -> Result {
    let (mut runtime, tile) = fixture()?;
    let cost = runtime
        .execute_command(&clear(tile, CommandMode::Estimate))?
        .result
        .ok_or("estimate")?
        .cost;
    assert!(cost > 0);
    for money in [cost - 1, cost] {
        let mut world = World::decode(&runtime.to_savegame()?)?;
        world.edit_batch(vec![WorldEdit::Field {
            chunk: *b"PLYR",
            record: 0,
            path: vec![PathElement::Field("money".into())],
            value: WireValue::Signed(money),
        }])?;
        let mut candidate = SimulationRuntime::restore_vanilla(world)?;
        let before = candidate.world().saved_json()?;
        let pool = candidate.depot_pool();
        let receipt = candidate.execute_command(&clear(tile, CommandMode::Post))?;
        assert!(receipt.test.ok_or("test")?.success);
        assert_eq!(receipt.posted, money == cost);
        if money < cost {
            assert_eq!(
                receipt.result.ok_or("result")?.error.as_deref(),
                Some("STR_ERROR_NOT_ENOUGH_CASH_REQUIRES_CURRENCY")
            );
            assert!(receipt.exec.is_none());
            assert_eq!(candidate.world().saved_json()?, before);
            assert_eq!(candidate.depot_pool(), pool);
        } else {
            assert!(receipt.exec.is_some());
            assert_eq!(candidate.depot_pool().items, 0);
        }
    }
    Ok(())
}
