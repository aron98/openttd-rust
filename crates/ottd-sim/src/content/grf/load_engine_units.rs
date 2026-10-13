use super::super::{LoadLocation, LoadStage, load_engine_mapping::Kind, load_specs::Specs};
use super::{ControlOptions, Result, programs, run};
use serde_json::{Value, json};

fn location() -> LoadLocation {
    LoadLocation {
        stage: LoadStage::Activation,
        file: 0,
        line: 1,
        offset: 0,
    }
}

pub(super) fn projection(state: &Specs) -> Result<Value> {
    let owners = state.owners.iter().map(|owner| {
        let mut info = serde_json::to_value(owner.spec.info)?;
        info.as_object_mut().ok_or("info object")?.insert("string_id".into(), json!(owner.string_id));
        Ok(json!({"id":owner.spec.id,"type":owner.kind.index(),"local_id":owner.spec.local_id,
            "grfid":owner.grfid,"stored_grfid":owner.stored_grfid,"info":info,"vehicle":owner.spec.vehicle,
            "list_position":owner.spec.local_id,"original_image_index":owner.original_image_index,
            "badges":owner.badges,"spritegroups":[],"wagon_overrides":[],"dynamic":owner.dynamic}))
    }).collect::<Result<Vec<_>>>()?;
    let mappings = state.mappings.entries.iter().flatten().map(|entry| json!({"bucket":entry.kind.index(),"type":entry.kind.index(),"grfid":entry.grfid,"internal_id":entry.internal_id,"substitute_id":entry.substitute_id,"engine":entry.engine})).collect::<Vec<_>>();
    let temporary = state
        .temporary
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let mut value = serde_json::to_value(value)?;
            value
                .as_object_mut()
                .ok_or("temporary object")?
                .insert("index".into(), json!(index));
            Ok(value)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"owners":owners,"mappings":mappings,"temporary":temporary,"grfid_overrides":state.grfid_overrides.iter().map(|(&a,&b)|[a,b]).collect::<Vec<_>>(),"pool_capacity":state.pool_capacity,"dynamic_engines":state.dynamic_engines}),
    )
}

fn expected(name: &str) -> Result<Value> {
    let values: Value = serde_json::from_str(include_str!("load_engine_native_states.json"))?;
    Ok(values.get(name).ok_or("native case")?.clone())
}

fn property(kind: u8, first: u16, raw: &[u8]) -> Vec<u8> {
    let mut result = vec![0, 1, 1, 1, 255];
    result.extend(first.to_le_bytes());
    result.push(kind);
    result.extend(raw);
    result
}

fn scalar_records() -> Vec<Vec<u8>> {
    [(8, 80), (9, 7), (15, 42), (17, 11)]
        .into_iter()
        .map(|(p, v)| property(p, 0, &[v]))
        .collect()
}

#[test]
fn engine_owners_reset_matches_all_original_native_fields() -> Result {
    let state = Specs::new(true, location())?;
    assert_eq!(projection(&state)?, expected("reset")?);
    Ok(())
}

#[test]
fn engine_scalar_loader_matches_native_prefinalization() -> Result {
    let source = programs::source(0x454e_4701, &scalar_records(), &[], 1)?;
    let (_, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        projection(&report.specs.ok_or("owners")?)?,
        expected("loader-scalars")?
    );
    Ok(())
}

#[test]
fn engine_truncated_loader_retains_owners_mapping_and_temporary_state() -> Result {
    let sources = [
        programs::source(0x454e_4701, &scalar_records(), &[], 1)?,
        programs::source(0x454e_4702, &[property(8, 1, &[])], &[], 1)?,
    ];
    let (_, report) = run(&sources, None, ControlOptions::default())?;
    assert_eq!(
        projection(&report.specs.ok_or("owners")?)?,
        expected("loader-truncated")?
    );
    Ok(())
}

#[test]
fn engine_unknown_loader_retains_owners_mapping_and_temporary_state() -> Result {
    let sources = [
        programs::source(0x454e_4701, &scalar_records(), &[], 1)?,
        programs::source(0x454e_4702, &[property(1, 2, &[99])], &[], 1)?,
    ];
    let (_, report) = run(&sources, None, ControlOptions::default())?;
    assert_eq!(
        projection(&report.specs.ok_or("owners")?)?,
        expected("loader-unknown")?
    );
    Ok(())
}

#[test]
fn engine_scoped_allocation_and_retained_reset_follow_original() -> Result {
    let mut state = Specs::new(true, location())?;
    assert_eq!(
        state.acquire(0x454e_4701, Kind::Road, 0, false, location())?,
        Some(116)
    );
    assert_eq!(
        state.acquire(0x454e_4702, Kind::Road, 0, false, location())?,
        Some(256)
    );
    assert_eq!(state.pool_capacity, 257);
    assert_eq!(projection(&state)?, expected("allocated-second-grf")?);
    state.acquire(0x454e_4701, Kind::Road, 1, false, location())?;
    state.acquire(0x454e_4701, Kind::Road, 2, false, location())?;
    let mappings = state.mappings.clone();
    state.reset(location())?;
    assert_eq!(state.mappings, mappings);
    assert!(state.owners.iter().all(|owner| owner.grfid.is_none()));
    assert_eq!(projection(&state)?, expected("reset-retained")?);
    assert_eq!(
        state.acquire(0x454e_4702, Kind::Road, 0, false, location())?,
        Some(256)
    );
    Ok(())
}

#[test]
fn engine_shared_scope_reuses_native_original_slot() -> Result {
    let mut state = Specs::new(false, location())?;
    assert_eq!(
        state.acquire(1, Kind::Road, 0, false, location())?,
        Some(116)
    );
    assert_eq!(
        state.acquire(2, Kind::Road, 0, false, location())?,
        Some(116)
    );
    assert_eq!(state.owners.len(), 256);
    Ok(())
}

#[test]
fn engine_static_access_does_not_reserve_or_allocate_mapping() -> Result {
    let mut state = Specs::new(true, location())?;
    let mappings = state.mappings.clone();
    assert_eq!(
        state.acquire(1, Kind::Road, 0, true, location())?,
        Some(116)
    );
    assert_eq!(state.mappings, mappings);
    assert_eq!(state.acquire(1, Kind::Road, 88, true, location())?, None);
    assert_eq!(state.owners.len(), 256);
    Ok(())
}

#[test]
fn engine_partial_multi_owner_read_retains_first_write_and_second_allocation() -> Result {
    let mut action = property(8, 0, &[80]);
    *action.get_mut(3).ok_or("count")? = 2;
    let source = programs::source(0x454e_4701, &[action], &[], 1)?;
    let (control, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        control.files.first().ok_or("file")?.status,
        super::LoadStatus::Disabled
    );
    let state = report.specs.ok_or("owners")?;
    let first = state.owners.get(116).ok_or("first")?;
    let crate::content::VehicleSpec::Road(road) = first.spec.vehicle else {
        return Err("road".into());
    };
    assert_eq!(road.max_speed, 80);
    assert_eq!(
        state.owners.get(117).ok_or("second")?.grfid,
        Some(0x454e_4701)
    );
    Ok(())
}

#[test]
fn engine_unknown_multi_owner_property_allocates_all_before_disabling() -> Result {
    let mut action = property(1, 0, &[99]);
    *action.get_mut(3).ok_or("count")? = 2;
    let source = programs::source(0x454e_4701, &[action], &[], 1)?;
    let (control, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        control.files.first().ok_or("file")?.status,
        super::LoadStatus::Disabled
    );
    let state = report.specs.ok_or("owners")?;
    assert_eq!(
        state.owners.get(116).ok_or("first")?.grfid,
        Some(0x454e_4701)
    );
    assert_eq!(
        state.owners.get(117).ok_or("second")?.grfid,
        Some(0x454e_4701)
    );
    Ok(())
}

#[test]
fn engine_trace_budget_remains_host_error() -> Result {
    let source = programs::source(1, &[property(8, 0, &[80])], &[], 1)?;
    let result = run(
        &[source],
        None,
        ControlOptions {
            max_trace_bytes: 1024,
            ..ControlOptions::default()
        },
    );
    assert!(
        matches!(result,Err(error) if matches!(error.downcast_ref::<super::ControlLoadError>(),Some(super::ControlLoadError::ResourceLimit { .. })))
    );
    Ok(())
}

#[test]
fn engine_continuation_unknown_stops_later_record() -> Result {
    let source = programs::source(
        0x454e_4701,
        &[
            property(8, 0, &[80]),
            property(1, 0, &[99]),
            property(8, 0, &[111]),
        ],
        &[],
        1,
    )?;
    let (control, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        control.files.first().ok_or("configured file")?.status,
        super::LoadStatus::Disabled
    );
    let state = projection(&report.specs.ok_or("owners")?)?;
    assert_eq!(
        state
            .pointer("/owners/116/vehicle/Road/max_speed")
            .ok_or("road speed")?,
        &json!(80)
    );
    Ok(())
}

#[test]
fn engine_continuation_recognized_prefix_keeps_later_record() -> Result {
    let source = programs::source(
        0x454e_4701,
        &[property(8, 0, &[80]), property(8, 0, &[111])],
        &[],
        1,
    )?;
    let (control, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        control.files.first().ok_or("configured file")?.status,
        super::LoadStatus::Activated
    );
    let state = projection(&report.specs.ok_or("owners")?)?;
    assert_eq!(
        state
            .pointer("/owners/116/vehicle/Road/max_speed")
            .ok_or("road speed")?,
        &json!(111)
    );
    Ok(())
}
