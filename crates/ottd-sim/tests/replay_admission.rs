//! Admission belongs to executed replay actions, not populated load or future work.
use ottd_save::{Savegame, world::World};
use ottd_sim::{ReplayCursor, ReplayError, ReplayEvent, ReplayPlan, advance_world, run_replay};
use serde_json::{Value, json};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn world(moving: bool) -> Result<World> {
    let bytes: &[u8] = if moving {
        include_bytes!("../../../fixtures/road-movement/bus-first.sav")
    } else {
        include_bytes!("../../../fixtures/replay/populated-v362.sav")
    };
    Ok(World::decode(&Savegame::decode(
        bytes,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

fn cursor(actions: &Value) -> Result<ReplayCursor> {
    let plan: ReplayPlan = serde_json::from_value(json!({"schema_version":1,"actions":actions}))?;
    Ok(ReplayCursor::new(plan)?)
}

fn pause(ordinal: u64, paused: bool) -> Value {
    json!({"ordinal":ordinal,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"pause","mode":0,"paused":paused}}})
}

#[test]
fn nonroad_empty_checkpoint_and_zero_tick_plans_preserve_world() -> Result {
    let world = world(false)?;
    let before = world.saved_json()?;
    for actions in [
        json!([]),
        json!([{"ordinal":0,"op":"checkpoint","label":"saved"}]),
        json!([{"ordinal":0,"op":"tick","count":0}]),
    ] {
        let cursor = cursor(&actions)?;
        let outcome = run_replay(&world, &cursor, None, &mut |_, _| Ok(()))?;
        assert_eq!(outcome.world.saved_json()?, before);
        assert_eq!(outcome.cursor.remaining(), 0);
        assert_eq!(world.saved_json()?, before);
        assert_eq!(cursor.position(), 0);
    }
    Ok(())
}

#[test]
fn paused_nonroad_calls_use_existing_world_scheduler() -> Result {
    let initial = world(false)?;
    let original = initial.saved_json()?;
    let prefix = run_replay(
        &initial,
        &cursor(&json!([pause(0, true)]))?,
        None,
        &mut |_, _| Ok(()),
    )?;
    let mut expected = prefix.world;
    let report = advance_world(&mut expected, 3)?;
    assert_eq!(report.ticks.len(), 3);
    let outcome = run_replay(
        &initial,
        &cursor(&json!([pause(0, true), {"ordinal":1,"op":"tick","count":3}]))?,
        None,
        &mut |_, _| Ok(()),
    )?;
    assert_eq!(outcome.world.saved_json()?, expected.saved_json()?);
    assert_eq!(initial.saved_json()?, original);
    Ok(())
}

#[test]
fn future_unsupported_tick_does_not_preempt_through_or_resume_prefix() -> Result {
    let initial = world(false)?;
    let before = initial.saved_json()?;
    let plan = cursor(&json!([
        {"ordinal":10,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"rename_company","text":"Prefix admitted"}}},
        {"ordinal":20,"op":"checkpoint","label":"prefix"},
        pause(30, false),
        {"ordinal":40,"op":"tick","count":1}
    ]))?;
    let first = run_replay(&initial, &plan, Some(10), &mut |_, _| Ok(()))?;
    assert_eq!(first.cursor.remaining(), 3);
    let restored: ReplayCursor = serde_json::from_slice(&serde_json::to_vec(&first.cursor)?)?;
    let second = run_replay(&first.world, &restored, Some(20), &mut |_, _| Ok(()))?;
    assert_eq!(second.cursor.remaining(), 2);
    assert_eq!(second.world.saved_json()?, first.world.saved_json()?);
    let candidate_before = second.world.saved_json()?;
    let result = run_replay(&second.world, &second.cursor, None, &mut |_, _| Ok(()));
    assert!(matches!(result, Err(ReplayError::Runtime(_))));
    assert_eq!(second.world.saved_json()?, candidate_before);
    assert_eq!(second.cursor.remaining(), 2);
    assert_eq!(initial.saved_json()?, before);
    assert_eq!(plan.position(), 0);
    Ok(())
}

#[test]
fn actual_nonroad_movement_refuses_after_prior_command_observation() -> Result {
    let initial = world(false)?;
    let before = initial.saved_json()?;
    let plan = cursor(&json!([pause(0, false), {"ordinal":1,"op":"tick","count":1}]))?;
    let mut actions = Vec::new();
    let result = run_replay(&initial, &plan, None, &mut |event, _| {
        if let ReplayEvent::Action(action) = event {
            actions.push(action.op);
        }
        Ok(())
    });
    assert!(matches!(result, Err(ReplayError::Runtime(_))));
    assert_eq!(actions, ["command"]);
    assert_eq!(initial.saved_json()?, before);
    assert_eq!(plan.position(), 0);
    Ok(())
}

#[test]
fn observer_failure_after_saved_command_rolls_back_input_and_cursor() -> Result {
    let initial = world(false)?;
    let before = initial.saved_json()?;
    let plan = cursor(
        &json!([{"ordinal":0,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"rename_company","text":"Private candidate"}}}]),
    )?;
    let mut saw_candidate = false;
    let result = run_replay(&initial, &plan, None, &mut |event, observed| {
        if let ReplayEvent::Action(_) = event {
            saw_candidate = observed
                .saved_json()
                .map_err(|e| ReplayError::Observer(e.to_string()))?
                != before;
            return Err(ReplayError::Observer("staged observation failed".into()));
        }
        Ok(())
    });
    assert!(
        matches!(result, Err(ReplayError::Observer(ref message)) if message == "staged observation failed")
    );
    assert!(saw_candidate);
    assert_eq!(initial.saved_json()?, before);
    assert_eq!(plan.position(), 0);
    Ok(())
}

#[test]
fn road_ticks_service_and_encoded_resume_keep_same_world() -> Result {
    let initial = world(true)?;
    let before = initial.saved_json()?;
    let plan = cursor(&json!([
        {"ordinal":0,"op":"tick","count":37},
        {"ordinal":1,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"change_service_interval","vehicle":0,"interval":150,"custom":false,"percent":false}}},
        {"ordinal":2,"op":"tick","count":0}, {"ordinal":3,"op":"tick","count":37}
    ]))?;
    let mut posted = false;
    let continuous = run_replay(&initial, &plan, None, &mut |event, _| {
        if let ReplayEvent::Action(action) = event {
            posted = action
                .receipt
                .as_ref()
                .map_or(posted, |receipt| receipt.posted);
        }
        Ok(())
    })?;
    assert!(posted);
    let first = run_replay(&initial, &plan, Some(0), &mut |_, _| Ok(()))?;
    let encoded = first.world.encode(ottd_save::Compression::Zlib)?;
    let loaded = World::decode(&Savegame::decode(&encoded, ottd_save::DEFAULT_MAX_BYTES)?)?;
    let resumed = run_replay(&loaded, &first.cursor, None, &mut |_, _| Ok(()))?;
    assert_eq!(continuous.world.saved_json()?, resumed.world.saved_json()?);
    assert_ne!(continuous.world.saved_json()?, before);
    assert_eq!(initial.saved_json()?, before);
    Ok(())
}

#[test]
fn post_promotion_pause_remains_explicitly_unsupported() -> Result {
    let initial = world(true)?;
    let before = initial.saved_json()?;
    let plan = cursor(&json!([{"ordinal":0,"op":"tick","count":37}, pause(1, true)]))?;
    let mut observed = Vec::new();
    let result = run_replay(&initial, &plan, None, &mut |event, _| {
        if let ReplayEvent::Action(action) = event {
            observed.push(action.op);
        }
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ReplayError::Command(ottd_sim::CommandError::Unsupported(
            "runtime command cache publication"
        )))
    ));
    assert_eq!(observed, ["tick"]);
    assert_eq!(initial.saved_json()?, before);
    assert_eq!(plan.position(), 0);
    Ok(())
}
