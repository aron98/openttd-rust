//! Replay original scratch-pool operations and compare every logical state.
use ottd_sim::runtime::pools::{PoolAllocator, PoolError, PoolSnapshot, UnitNumberAllocator};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum PoolAction {
    Insert,
    Allocate,
    Free,
    Reset,
    CanAllocate,
}
#[derive(Debug, Deserialize, Serialize)]
struct PoolStep {
    action: PoolAction,
    id: u32,
    result: serde_json::Value,
    state: serde_json::Value,
}
#[derive(Debug, Deserialize, Serialize)]
struct PoolTrace {
    limit: u32,
    growth: u32,
    trace: Vec<PoolStep>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum UnitAction {
    Use,
    Release,
    Fill,
    Next,
}
#[derive(Debug, Deserialize, Serialize)]
struct UnitStep {
    company: u8,
    #[serde(rename = "type")]
    kind: u8,
    action: UnitAction,
    id: u16,
    result: serde_json::Value,
    next: u16,
}
#[derive(Debug, Deserialize, Serialize)]
struct Trace {
    schema_version: u32,
    pools: Vec<PoolTrace>,
    units: Vec<UnitStep>,
}

fn compare(actual: &serde_json::Value, expected: &serde_json::Value) -> Result {
    if actual != expected {
        return Err("native allocation trace differs".into());
    }
    Ok(())
}

fn replay(trace: &mut Trace) -> Result {
    for case in &mut trace.pools {
        let mut pool = PoolAllocator::new(case.limit, case.growth)?;
        for step in &mut case.trace {
            step.result = match step.action {
                PoolAction::Insert => {
                    pool.insert(step.id)?;
                    step.id.into()
                }
                PoolAction::Allocate => match pool.allocate() {
                    Ok(id) => id.into(),
                    Err(PoolError::Exhausted) => serde_json::Value::Null,
                    Err(error) => return Err(error.into()),
                },
                PoolAction::Free => {
                    pool.free(step.id)?;
                    serde_json::Value::Null
                }
                PoolAction::Reset => {
                    pool.reset();
                    serde_json::Value::Null
                }
                PoolAction::CanAllocate => pool.can_allocate(step.id).into(),
            };
            step.state = serde_json::to_value(pool.snapshot())?;
        }
    }
    let mut units: BTreeMap<(u8, u8), UnitNumberAllocator> = BTreeMap::new();
    for step in &mut trace.units {
        let unit = units.entry((step.company, step.kind)).or_default();
        step.result = match step.action {
            UnitAction::Use => unit.use_id(step.id).into(),
            UnitAction::Release => {
                unit.release_id(step.id)?;
                serde_json::Value::Null
            }
            UnitAction::Fill => {
                for id in 1..65535 {
                    unit.use_id(id);
                }
                serde_json::Value::Null
            }
            UnitAction::Next => serde_json::Value::Null,
        };
        step.next = unit.next_id();
    }
    Ok(())
}

#[test]
#[ignore = "requires fresh original trace; set OTTD_ALLOCATION_JSON and OTTD_ALLOCATION_EVIDENCE"]
fn original_pool_and_unit_allocation_trace_matches() -> Result {
    let source = PathBuf::from(std::env::var("OTTD_ALLOCATION_JSON")?);
    let output = PathBuf::from(std::env::var("OTTD_ALLOCATION_EVIDENCE")?);
    std::fs::create_dir(&output)?;
    let native: serde_json::Value = serde_json::from_slice(&std::fs::read(source)?)?;
    let mut trace: Trace = serde_json::from_value(native.clone())?;
    assert_eq!(trace.schema_version, 1);
    assert_eq!(trace.pools.len(), 2);
    assert!(trace.pools.iter().all(|case| case.trace.len() > 512));
    replay(&mut trace)?;
    let actual = serde_json::to_value(&trace)?;
    std::fs::write(
        output.join("rust.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    compare(&actual, &native)?;
    let first = trace
        .pools
        .first_mut()
        .and_then(|case| case.trace.first_mut())
        .ok_or("first pool step")?;
    let mut state: PoolSnapshot = serde_json::from_value(first.state.clone())?;
    state.first_free ^= 1;
    first.state = serde_json::to_value(state)?;
    let wrong = serde_json::to_value(&trace)?;
    assert!(compare(&wrong, &native).is_err());
    std::fs::write(
        output.join("negative-cursor.json"),
        serde_json::to_vec_pretty(&wrong)?,
    )?;
    std::fs::write(
        output.join("comparison.log"),
        "PASS original scratch Pool and FreeUnitIDGenerator traces; REJECT altered first_free\n",
    )?;
    Ok(())
}
