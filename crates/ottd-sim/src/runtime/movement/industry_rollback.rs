//! Historical year-refusal selector now checks the supported genuine wood continuation.
use crate::runtime::SimulationRuntime;
use ottd_save::{Savegame, world::World};
use serde_json::json;
use std::path::PathBuf;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires WOOD_ROLLBACK_NATIVE/OUTPUT; historical selector, completion branch only"]
fn genuine_wood_handoff_loaded_prefix_then_year_refusal_preserves_worker_owner() -> Result {
    let native = PathBuf::from(std::env::var("WOOD_ROLLBACK_NATIVE")?);
    let output = PathBuf::from(std::env::var("WOOD_ROLLBACK_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    // Given actual native tick3080 with its saved asynchronous job and no wood packet.
    let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &std::fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    super::year_native::compare(&runtime, &native, "initial")?;
    assert_eq!(runtime.advance_world(23_929)?.ticks.len(), 23_929);
    super::year_native::compare(&runtime, &native, "prefix")?;
    assert_eq!(
        runtime
            .transport
            .cargo
            .vehicles
            .get(&0)
            .ok_or("wood truck")?
            .count,
        24
    );
    // When the native completed branch reaches its natural year boundary.
    assert_eq!(runtime.advance_world(1)?.ticks.len(), 1);
    // Then all compared native state and owned group-profit fields match.
    super::year_native::compare(&runtime, &native, "final")?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&json!({
            "synthetic":false,"input_tick":3080,"positive_calls":23929,"whole_calls":23930,
            "year_supported":true,"whole_world_derived_nine_views_three_group_fields":true,
            "rollback_credit":false,"scope":"observed completion branch only; pending branch remains required"
        }))?,
    )?;
    Ok(())
}
