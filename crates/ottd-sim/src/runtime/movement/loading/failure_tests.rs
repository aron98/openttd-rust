//! Late negative probes use genuine prepared saves; cache faults are labelled explicitly.
use crate::runtime::{SimulationRuntime, pools::PoolAllocator};
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use std::path::PathBuf;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn load(variable: &str) -> Result<SimulationRuntime> {
    let bytes = std::fs::read(std::env::var(variable)?)?;
    Ok(SimulationRuntime::restore_vanilla(World::decode(
        &Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?,
    )?)?)
}
fn refusal(
    mut runtime: SimulationRuntime,
    case: (&str, u32, &str),
    output: &std::path::Path,
) -> Result {
    let (name, ticks, expected) = case;
    let before = format!("{runtime:?}");
    let saved = runtime.saved_json()?;
    let result = runtime.advance_world(ticks);
    let error = format!("{result:?}");
    std::fs::write(
        output.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ticks":ticks,"result":error,"entire_runtime_unchanged":before==format!("{runtime:?}"),
            "entire_saved_world_unchanged":saved==runtime.saved_json()?,"probe":name,
        }))?,
    )?;
    assert!(
        result.is_err(),
        "required negative probe unexpectedly succeeded"
    );
    assert!(error.contains(expected), "unexpected boundary: {error}");
    assert_eq!(format!("{runtime:?}"), before);
    assert_eq!(runtime.saved_json()?, saved);
    Ok(())
}
#[test]
#[ignore = "requires native command-created prepared and running-job saves"]
fn terminal_late_pool_order_job_failures_are_atomic() -> Result {
    let output = PathBuf::from(std::env::var("ROAD_TERMINAL_FAILURE_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    // Given: genuine original prepared routes, with deliberately exhausted allocators.
    let mut payment = load("ROAD_TERMINAL_PREPARED")?;
    payment.transport.payments = PoolAllocator::restore(1, 1, [0])?;
    // When / Then: first arrival cannot allocate its payment after 508 tentative ticks.
    refusal(
        payment,
        ("payment-allocator-fault", 508, "Exhausted"),
        &output,
    )?;
    let mut graph = load("ROAD_TERMINAL_PREPARED")?;
    graph.transport.graphs = PoolAllocator::restore(1, 1, [0])?;
    refusal(graph, ("graph-allocator-fault", 509, "Exhausted"), &output)?;
    // Given: a valid future timetable start, outside ordinary measured-order capability.
    let mut order = load("ROAD_TERMINAL_PREPARED")?;
    order.world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"VEHS",
        record: 0,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field("timetable_start".into()),
        ],
        value: WireValue::Unsigned(767_040),
    }])?;
    refusal(
        order,
        (
            "future-timetable-start",
            508,
            "scheduled or autofill timetable",
        ),
        &output,
    )?;
    // Given: native bay/loading state with an explicitly fault-injected free-bay bit.
    // Loading/payment destruction and graph allocation precede the controller's bay check.
    let mut bay = load("ROAD_TERMINAL_LOADING")?;
    bay.world.edit_batch(vec![WorldEdit::Field {
        chunk: *b"ROAD",
        record: 0,
        path: vec![PathElement::Field("status".into())],
        value: WireValue::Unsigned(3),
    }])?;
    refusal(
        bay,
        (
            "late-bay-occupancy-fault",
            1,
            "saved bay occupancy or entry state",
        ),
        &output,
    )?;
    // Given: native saved running job, with a deliberately inconsistent derived result.
    let mut job = load("ROAD_TERMINAL_RUNNING_JOB")?;
    job.transport
        .job_results
        .get_mut(&0)
        .ok_or("job cache")?
        .as_mut()
        .ok_or("empty job result")?
        .edges = 999;
    refusal(
        job,
        (
            "late-job-result-cache-fault",
            2682,
            "pending job result cache",
        ),
        &output,
    )?;
    Ok(())
}
