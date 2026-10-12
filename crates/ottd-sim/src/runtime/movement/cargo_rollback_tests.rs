//! Complete native positive prefix precedes a labelled late candidate-cache fault.
use super::{RoadPhases, SavedVehicleView, State, spatial, stations};
use crate::runtime::{SimulationRuntime, pools::PoolAllocator};
use ottd_save::{Savegame, world::World};
use std::path::{Path, PathBuf};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn load(path: &Path) -> Result<SimulationRuntime> {
    Ok(SimulationRuntime::restore_vanilla(World::decode(
        &Savegame::decode(&std::fs::read(path)?, ottd_save::DEFAULT_MAX_BYTES)?,
    )?)?)
}
#[test]
#[ignore = "requires CARGO_ROLLBACK_INPUT, CARGO_ROLLBACK_NATIVE, CARGO_ROLLBACK_OUTPUT genuine 8192-call positive route"]
fn transported_and_joined_prefix_then_late_cache_fault_is_atomic() -> Result {
    let input = PathBuf::from(std::env::var("CARGO_ROLLBACK_INPUT")?);
    let native = PathBuf::from(std::env::var("CARGO_ROLLBACK_NATIVE")?);
    let output = PathBuf::from(std::env::var("CARGO_ROLLBACK_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let expected = World::decode(&Savegame::decode(
        &std::fs::read(native.join("native/final.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let observed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(native.join("native-last-tick.json"))?)?;
    let mut runtime = load(&input)?;
    let before = format!("{runtime:?}");
    let saved = runtime.saved_json()?;
    let derived = serde_json::to_value(runtime.world.derived())?;
    let live = runtime.transport_state_json();
    {
        let (&id, cache) = runtime.road.first_key_value().ok_or("road cache")?;
        let mut runner = RoadPhases {
            id,
            cache: cache.clone(),
            content: &runtime.content,
            spatial: spatial::Spatial::restore(&runtime.world, id)?,
            stations: stations::IdleStations::restore(&runtime.world, &runtime.stations)?,
            station_context: &runtime.stations,
            transport: runtime.transport.clone(),
            group_profits: runtime.group_profits.clone(),
            company_ratings: runtime.company_ratings.clone(),
        };
        let (candidate, report, towns) = State::plan(&mut runtime.world, 8192, &mut runner)?;
        assert_eq!(report.ticks.len(), 8192);
        runner.spatial.validate(
            SavedVehicleView::candidate(candidate.view(), id)?,
            candidate.map().width(),
        )?;
        runner.transport.validate(candidate.view())?;
        for (chunk, table) in expected.tables() {
            let actual = candidate.view().table(*chunk).ok_or("candidate table")?;
            assert_eq!(actual.schema(), table.schema(), "{chunk:?}");
            let rows = actual
                .records()
                .map(|(id, row)| (id, row.clone()))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(&rows, table.records(), "{chunk:?}");
        }
        for (index, tile) in expected.map().tiles().iter().enumerate() {
            assert_eq!(candidate.map().tile(u32::try_from(index)?), Some(tile));
        }
        assert_eq!(
            serde_json::to_value(candidate.derived())?,
            serde_json::to_value(expected.derived())?
        );
        let actual_transport = runner.transport.snapshot();
        assert_eq!(
            Some(&actual_transport),
            observed.pointer("/movement/transport")
        );
        assert_eq!(
            Some(&serde_json::to_value(towns.as_ref().ok_or("town caches")?)?),
            observed.pointer("/movement/house_caches")
        );
        assert_eq!(
            Some(&serde_json::to_value(&runner.cache)?),
            observed.pointer("/movement/runtime_road/road/0")
        );
        std::fs::write(
            output.join("positive-transport.json"),
            serde_json::to_vec_pretty(&actual_transport)?,
        )?;
        std::fs::write(
            output.join("positive-house-caches.json"),
            serde_json::to_vec_pretty(&towns)?,
        )?;
        std::fs::write(
            output.join("positive-derived.json"),
            serde_json::to_vec(candidate.derived())?,
        )?;
        // Fault is introduced only after the complete native prefix has been verified.
        let extra = runner.transport.cargo.pool.allocate()?;
        let error = runner
            .transport
            .validate(candidate.view())
            .expect_err("unpublished packet allocation must fail validation");
        assert!(format!("{error:?}").contains("candidate cargo pool membership"));
        std::fs::write(
            output.join("late-fault.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "synthetic":true,"after_positive_calls":8192,"unpublished_packet_id":extra,
                "error":format!("{error:?}"),"complete_native_candidate_tables_map_derived_road_transport_house_equal_before_fault":true
            }))?,
        )?;
    }
    assert_eq!(runtime.saved_json()?, saved);
    assert_eq!(serde_json::to_value(runtime.world.derived())?, derived);
    assert_eq!(format!("{runtime:?}"), before);
    assert_eq!(runtime.transport_state_json(), live);
    public_validation_failure(&input, &output)?;
    std::fs::write(output.join("PASS.json"),b"{\"positive_prefix_before_fault\":8192,\"complete_committed_world_derived_runtime_cache_unchanged\":true,\"public_advance_late_validation_failure_unchanged\":true}")?;
    Ok(())
}
fn public_validation_failure(input: &Path, output: &Path) -> Result {
    let mut runtime = load(input)?;
    // Separate synthetic initial allocator inconsistency, detected by the public
    // advance_world final validation after all requested ticks; no native parity credit.
    runtime.transport.cargo.pool = PoolAllocator::restore(0xff_f000, 1024, [700])?;
    let before = format!("{runtime:?}");
    let saved = runtime.saved_json()?;
    let live = runtime.transport_state_json();
    let error = runtime
        .advance_world(8192)
        .expect_err("late candidate membership fault");
    assert!(format!("{error:?}").contains("candidate cargo pool membership"));
    assert_eq!(runtime.saved_json()?, saved);
    assert_eq!(format!("{runtime:?}"), before);
    assert_eq!(runtime.transport_state_json(), live);
    std::fs::write(
        output.join("public-failure.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "synthetic":true,"requested_ticks":8192,"result":format!("{error:?}"),"entire_committed_state_unchanged":true
        }))?,
    )?;
    Ok(())
}
