//! Source-ordered scheduling and publication of zero-supply link graph jobs.
use super::{RoadPhases, State, WorldTickError, records, unsupported};
use crate::runtime::{RuntimeError, transport::empty_job, transport_jobs::EmptyInput};
use ottd_save::{WireValue, world::WorldEdit};

pub(in crate::runtime::movement) fn tick(
    state: &mut State<'_>,
    runner: &mut RoadPhases<'_>,
) -> Result<(), WorldTickError> {
    if state.number(b"DATE", 0, "economy_date_fract")? != 21 {
        return Ok(());
    }
    let interval = state.number(b"PATS", 0, "linkgraph.recalc_interval")? / 2;
    if interval <= 0 {
        return Err(unsupported("link_jobs", "recalculation interval"));
    }
    let offset = state
        .number(b"DATE", 0, "economy_date")?
        .checked_rem(interval)
        .ok_or_else(|| unsupported("link_jobs", "recalculation interval"))?;
    if offset == 0 {
        spawn(state, runner)?;
    } else if offset == interval / 2 {
        join(state, runner)?;
    }
    Ok(())
}
fn list(state: &State<'_>, name: &str) -> Result<Vec<WireValue>, WorldTickError> {
    let WireValue::Array(values) = state.value(b"LGRS", 0, name)? else {
        return Err(unsupported("link_jobs", "schedule list"));
    };
    Ok(values.clone())
}
fn index(pointer: &WireValue) -> Result<u32, WorldTickError> {
    u32::try_from(
        State::number_value(pointer)?
            .checked_sub(1)
            .ok_or_else(|| unsupported("link_jobs", "null scheduled pointer"))?,
    )
    .map_err(|_| unsupported("link_jobs", "scheduled pointer"))
}
fn spawn(state: &mut State<'_>, runner: &mut RoadPhases<'_>) -> Result<(), WorldTickError> {
    let mut schedule = list(state, "schedule")?;
    let mut selected = None;
    for _ in 0..schedule.len() {
        let graph = index(
            schedule
                .first()
                .ok_or_else(|| unsupported("link_jobs", "schedule front"))?,
        )?;
        if records::row(state, *b"LGRP", graph)?.rows("nodes")?.1.len() >= 2 {
            selected = Some(graph);
            break;
        }
        schedule.rotate_left(1);
    }
    let Some(graph) = selected else {
        return Ok(());
    };
    schedule.remove(0);
    records::schedule(state, "schedule", schedule)?;
    let id = runner
        .transport
        .jobs
        .allocate()
        .map_err(RuntimeError::from)?;
    let source = records::row(state, *b"LGRP", graph)?;
    let result =
        empty_job(source)?.ok_or_else(|| unsupported("link_jobs", "populated job solver"))?;
    let schema = state.table(b"LGRJ")?.schema();
    let graph_schema = records::child(schema, "linkgraph")?;
    if graph_schema != source.schema {
        return Err(unsupported("link_jobs", "copied graph schema"));
    }
    let mut fields = Vec::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let value = match field.name() {
            "linkgraph" => WireValue::Structs(vec![source.record.clone()]),
            "link_graph.index" => WireValue::Unsigned(u64::from(graph)),
            "join_date" => WireValue::Signed(
                state
                    .number(b"DATE", 0, "economy_date")?
                    .wrapping_add(state.number(b"PATS", 0, "linkgraph.recalc_time")? / 2),
            ),
            name if name.starts_with("linkgraph.") => state.value(b"PATS", 0, name)?.clone(),
            _ => return Err(unsupported("link_jobs", "constructor field")),
        };
        fields.push((field.name(), value));
    }
    let record = records::construct(schema, &fields)?;
    state.apply(WorldEdit::InsertRecord {
        chunk: *b"LGRJ",
        record: id,
        value: record,
    })?;
    let input = EmptyInput::parse(records::row(state, *b"LGRJ", id)?.child("linkgraph")?)?
        .ok_or_else(|| unsupported("link_jobs", "populated job solver"))?;
    runner.transport.workers.insert(id, input.spawn());
    runner.transport.job_expectations.insert(id, Some(result));
    let mut running = list(state, "running")?;
    running.push(WireValue::Unsigned(u64::from(id).wrapping_add(1)));
    records::schedule(state, "running", running)
}
fn join(state: &mut State<'_>, runner: &mut RoadPhases<'_>) -> Result<(), WorldTickError> {
    let mut running = list(state, "running")?;
    let Some(front) = running.first() else {
        return Ok(());
    };
    let job = index(front)?;
    if state.number(b"LGRJ", job, "join_date")? > state.number(b"DATE", 0, "economy_date")? {
        return Ok(());
    }
    let row = records::row(state, *b"LGRJ", job)?;
    let graph = u32::try_from(row.number("link_graph.index")?)
        .map_err(|_| unsupported("link_jobs", "graph index"))?;
    let result = empty_job(row.child("linkgraph")?)?
        .ok_or_else(|| unsupported("link_jobs", "populated job result"))?;
    if runner.transport.job_expectations.get(&job) != Some(&Some(result.clone())) {
        return Err(unsupported("link_jobs", "pending job result cache"));
    }
    let completed = runner
        .transport
        .workers
        .get(&job)
        .ok_or_else(|| unsupported("link_jobs", "missing job worker"))?
        .wait()
        .map_err(RuntimeError::from)?;
    if *completed != result {
        return Err(unsupported("link_jobs", "worker result differs"));
    }
    // Empty source/destination demand yields no MCF paths or FlowMapper flows.
    // Admission also requires empty station flow/cargo state, so joining changes no goods data.
    running.remove(0);
    records::schedule(state, "running", running)?;
    state.apply(WorldEdit::RemoveRecord {
        chunk: *b"LGRJ",
        record: job,
    })?;
    runner
        .transport
        .jobs
        .free(job)
        .map_err(RuntimeError::from)?;
    runner.transport.job_expectations.remove(&job);
    runner.transport.workers.remove(&job);
    if state.table(b"LGRP")?.record(graph).is_some() {
        let pointer = WireValue::Unsigned(u64::from(graph).wrapping_add(1));
        let mut schedule = list(state, "schedule")?;
        schedule.retain(|value| value != &pointer);
        schedule.push(pointer);
        records::schedule(state, "schedule", schedule)?;
    }
    Ok(())
}
