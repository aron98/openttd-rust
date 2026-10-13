//! Genuine later unsupported boundaries after a positively mutating owned-cache prefix.
use crate::runtime::{SimulationRuntime, job_executor::Backend};
use ottd_save::{Savegame, world::World};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn committed(runtime: &SimulationRuntime) -> Result<Value> {
    Ok(
        json!({"complete_runtime":format!("{runtime:?}"),"world":runtime.saved_json()?,
        "derived":runtime.world.derived(),"group_profits":runtime.group_profit_caches(),
        "content":runtime.content(),"transport":runtime.transport_state_json()}),
    )
}

#[test]
#[ignore = "requires genuine YEAR_ROLLBACK_NATIVE/OUTPUT/CALLS/REASON/WORKERS"]
fn genuine_late_boundary_preserves_world_owned_caches_and_worker_leases() -> Result {
    let native = PathBuf::from(std::env::var("YEAR_ROLLBACK_NATIVE")?);
    let output = PathBuf::from(std::env::var("YEAR_ROLLBACK_OUTPUT")?);
    let calls: u32 = std::env::var("YEAR_ROLLBACK_CALLS")?.parse()?;
    let reason = std::env::var("YEAR_ROLLBACK_REASON")?;
    let workers: usize = std::env::var("YEAR_ROLLBACK_WORKERS")?.parse()?;
    std::fs::create_dir_all(&output)?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    // Given an unchanged original input and a successful whole-state prefix ending one call earlier.
    let mut positive = SimulationRuntime::restore_vanilla(world.clone())?;
    let initial = committed(&positive)?;
    super::year_native::compare(&positive, &native, "initial")?;
    let prefix = positive.advance_world(calls.checked_sub(1).ok_or("empty prefix")?);
    std::fs::write(
        output.join("prefix-result.json"),
        serde_json::to_vec_pretty(&json!({
            "success":prefix.is_ok(),"error":prefix.as_ref().err().map(ToString::to_string)
        }))?,
    )?;
    assert_eq!(prefix?.ticks.len(), usize::try_from(calls - 1)?);
    super::year_native::compare(&positive, &native, "prefix")?;
    assert_ne!(initial, committed(&positive)?);
    assert_ne!(
        initial
            .get("group_profits")
            .ok_or("initial group profits")?,
        &serde_json::to_value(positive.group_profit_caches())?
    );
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let owners = runtime
        .transport
        .workers
        .iter()
        .map(|(id, owner)| (*id, Arc::clone(owner)))
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), workers);
    let before = committed(&runtime)?;
    // When the same input executes the full request and reaches the valid native unsupported branch.
    let result = runtime.advance_world(calls);
    let after = committed(&runtime)?;
    for (label, value) in [("before", &before), ("after", &after)] {
        std::fs::write(
            output.join(format!("{label}.json")),
            serde_json::to_vec(value)?,
        )?;
    }
    std::fs::write(
        output.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "success":result.is_ok(),"calls":calls,"error":result.as_ref().err().map(ToString::to_string),
            "unchanged":before==after,"worker_count":workers
        }))?,
    )?;
    let error = result.expect_err("the declared later unsupported native branch must refuse");
    assert!(
        error
            .to_string()
            .contains(&format!("call {}: {reason}", calls - 1)),
        "{error}"
    );
    // Then all committed World/cache data and the exact original worker leases survive cleanup.
    assert_eq!(before, after);
    assert_eq!(runtime.transport.workers.len(), workers);
    for (id, owner) in owners {
        assert!(Arc::ptr_eq(
            &owner,
            runtime.transport.workers.get(&id).ok_or("lost worker")?
        ));
        assert_eq!(Arc::strong_count(&owner), 2);
        assert_eq!(owner.backend(), Backend::Threaded);
        assert!(owner.executing_thread().is_some());
        assert_ne!(owner.executing_thread(), Some(std::thread::current().id()));
    }
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&json!({
            "synthetic":false,"calls":calls,"positive_group_cache_prefix":true,
            "whole_committed_state_unchanged":true,"retained_threaded_worker_leases":workers
        }))?,
    )?;
    Ok(())
}
