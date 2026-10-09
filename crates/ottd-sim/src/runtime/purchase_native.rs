use super::*;
use crate::{ReplayAction, ReplayCursor, ReplayEvent, ReplayPlan, ReplayRuntime};
use ottd_save::{Compression, Savegame};
use serde_json::{Value, json};
use std::path::Path;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn observe(world: &World) -> Result<ReplayRuntime> {
    let empty = ReplayCursor::new(ReplayPlan {
        schema_version: 1,
        actions: Vec::new(),
    })?;
    let mut observed = None;
    crate::run_replay(world, &empty, None, &mut |event, _| {
        if let ReplayEvent::Checkpoint { runtime, .. } = event {
            observed = Some(runtime.clone());
        }
        Ok(())
    })?;
    observed.ok_or_else(|| "missing runtime observation".into())
}
fn checkpoint(runtime: &SimulationRuntime, label: &str, output: &Path) -> Result<Value> {
    let observed = observe(runtime.world())?;
    for (extension, value) in [
        ("world.json", runtime.world().saved_json()?),
        (
            "derived.json",
            serde_json::to_value(runtime.world().derived())?,
        ),
        (
            "runtime.json",
            json!({"before_save":observed,"after_save":observed}),
        ),
    ] {
        std::fs::write(
            output.join(format!("{label}.{extension}")),
            serde_json::to_vec(&value)?,
        )?;
    }
    std::fs::write(
        output.join(format!("{label}.sav")),
        runtime.world().to_savegame()?.encode(Compression::None)?,
    )?;
    Ok(json!({"label":label,"runtime":observed}))
}
fn live(runtime: &SimulationRuntime) -> Result<Value> {
    let mut units = Vec::new();
    for owner in runtime
        .world
        .tables()
        .get(b"PLYR")
        .ok_or("PLYR")?
        .records()
        .keys()
    {
        let owner = u8::try_from(*owner)?;
        units.push(json!({"company":owner,"next":runtime.allocation.road_units.get(&owner).map_or(1,pools::UnitNumberAllocator::next_id),"count":road_company_count(&runtime.world,owner)?}));
    }
    Ok(
        json!({"road":runtime.road.values().collect::<Vec<_>>(),"pool":runtime.allocation.pool.snapshot(),"units":units}),
    )
}
#[test]
#[ignore = "owned-runtime native purchase harness: PURCHASE_INPUT, PURCHASE_PLAN, PURCHASE_OUTPUT"]
fn run_native_purchase_sequence() -> Result {
    let output = std::path::PathBuf::from(std::env::var("PURCHASE_OUTPUT")?);
    std::fs::create_dir(&output)?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(std::env::var("PURCHASE_INPUT")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let plan: ReplayPlan =
        serde_json::from_slice(&std::fs::read(std::env::var("PURCHASE_PLAN")?)?)?;
    ReplayCursor::new(plan.clone())?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let mut actions = Vec::new();
    let mut checkpoints = vec![checkpoint(&runtime, "initial", &output)?];
    let mut caches = Vec::new();
    for action in plan.actions {
        let before = observe(runtime.world())?;
        let ordinal = action.ordinal();
        let observation = match action {
            ReplayAction::Command { request, .. } => {
                let pre = live(&runtime)?;
                let receipt = runtime.execute_command(&request)?;
                caches.push(json!({"ordinal":ordinal,"before":pre,"after":live(&runtime)?}));
                json!({"ordinal":ordinal,"op":"command","before":before,"after":observe(runtime.world())?,"receipt":receipt})
            }
            ReplayAction::Checkpoint { label, .. } => {
                checkpoints.push(checkpoint(&runtime, &label, &output)?);
                json!({"ordinal":ordinal,"op":"checkpoint","before":before,"after":observe(runtime.world())?})
            }
            ReplayAction::Tick { .. } => {
                return Err("purchase harness forbids vehicle ticks".into());
            }
        };
        actions.push(observation);
    }
    checkpoints.push(checkpoint(&runtime, "final", &output)?);
    std::fs::write(
        output.join("results.json"),
        serde_json::to_vec(
            &json!({"schema_version":1,"actions":actions,"checkpoints":checkpoints}),
        )?,
    )?;
    std::fs::write(output.join("live.json"), serde_json::to_vec(&caches)?)?;
    Ok(())
}
