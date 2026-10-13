//! Synthetic false capacity exercises real forest ordering without claiming a full native pool.
use super::{NewsQueue, RoadPhases, State, spatial, stations};
use crate::runtime::{
    SimulationRuntime, pools::PoolAllocator, stations::saved::StationRow,
    transport::cargo::read::Read,
};
use ottd_save::{Savegame, world::World};
use serde_json::json;
use std::path::PathBuf;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires SYNTHETIC_INDUSTRY_INPUT/OUTPUT; forced capacity predicate only, not a native full pool"]
fn synthetic_false_capacity_preserves_fraction_and_actual_transported_history() -> Result {
    let input = PathBuf::from(std::env::var("SYNTHETIC_INDUSTRY_INPUT")?);
    let output = PathBuf::from(std::env::var("SYNTHETIC_INDUSTRY_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &std::fs::read(input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    let before = runtime.saved_json()?;
    let committed = format!("{runtime:?}");
    {
        let (&id, cache) = runtime.road.first_key_value().ok_or("wood truck")?;
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
            news: NewsQueue::fresh_process(),
        };
        // Given a deliberately synthetic full one-item allocator, unrelated to actual native pool capacity.
        runner.transport.cargo.pool = PoolAllocator::restore(1, 1024, [0])?;
        assert!(!runner.transport.cargo.pool.can_allocate(1));
        // When the actual public planner executes the first forest handoff.
        let (candidate, report, _) = State::plan(&mut runtime.world, 1, &mut runner)?;
        assert_eq!(report.ticks.len(), 1);
        let read = Read::Candidate(candidate.view());
        let produced = read.row(*b"INDY", 0)?.child("produced")?;
        let (schema, history) = produced.rows("history")?;
        let history = StationRow {
            schema,
            record: history.first().ok_or("current history")?,
        };
        let (schema, goods) = read.row(*b"STNN", 0)?.child("normal")?.rows("goods")?;
        let goods = StationRow {
            schema,
            record: goods.get(7).ok_or("wood goods")?,
        };
        // Then stock is still drained before the later +13 tick, but no fractional/whole cargo is admitted.
        assert_eq!(produced.number("waiting")?, 13);
        assert_eq!(history.number("production")?, 156);
        assert_eq!(history.number("transported")?, 0);
        assert_eq!(goods.number("amount_fract")?, 0);
        assert!(read.ids(*b"CAPA")?.is_empty());
        assert_eq!(read.tile(649)?.m5(), 16);
        std::fs::write(
            output.join("capacity.json"),
            serde_json::to_vec_pretty(&json!({
                "synthetic":true,"full_native_pool_claim":false,"temporary_pool_limit":1,
                "temporary_pool_occupied":[0],
                "after_waiting":produced.number("waiting")?,"production":history.number("production")?,
                "transported":history.number("transported")?,"fraction":goods.number("amount_fract")?,
                "packets":read.ids(*b"CAPA")?,"graphics":read.tile(649)?.m5(),
            }))?,
        )?;
    }
    assert_eq!(before, runtime.saved_json()?);
    assert_eq!(committed, format!("{runtime:?}"));
    Ok(())
}
