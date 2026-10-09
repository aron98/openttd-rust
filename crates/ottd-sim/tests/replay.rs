//! Ordered replay and transactional rejection at the public library boundary.
use ottd_save::{Savegame, world::World};
use ottd_sim::{ReplayCursor, ReplayPlan, run_replay};
use serde_json::json;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
#[test]
fn ordered_names_resume_with_pending_actions_and_unchanged_input() -> Result {
    let world = world()?;
    let original = world.saved_json()?;
    let plan: ReplayPlan = serde_json::from_value(json!({"schema_version":1,"actions":[
        {"ordinal":10,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"rename_company","text":"First"}}},
        {"ordinal":20,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"rename_company","text":"Second"}}}
    ]}))?;
    let cursor = ReplayCursor::new(plan)?;
    let continuous = run_replay(&world, &cursor, None, &mut |_, _| Ok(()))?;
    let prefix = run_replay(&world, &cursor, Some(10), &mut |_, _| Ok(()))?;
    assert_eq!(prefix.cursor.next_ordinal(), 11);
    assert_eq!(prefix.cursor.remaining(), 1);
    let restored: ReplayCursor = serde_json::from_slice(&serde_json::to_vec(&prefix.cursor)?)?;
    let resumed = run_replay(&prefix.world, &restored, None, &mut |_, _| Ok(()))?;
    assert_eq!(resumed.world.saved_json()?, continuous.world.saved_json()?);
    assert_eq!(world.saved_json()?, original);
    Ok(())
}
#[test]
fn rejects_duplicate_ordinals_before_observing_world() -> Result {
    let plan: ReplayPlan = serde_json::from_value(
        json!({"schema_version":1,"actions":[{"ordinal":0,"op":"tick","count":0},{"ordinal":0,"op":"tick","count":0}]}),
    )?;
    assert!(ReplayCursor::new(plan).is_err());
    Ok(())
}
#[test]
fn unsupported_later_tick_does_not_change_input_world() -> Result {
    let world = world()?;
    let before = world.saved_json()?;
    let plan: ReplayPlan = serde_json::from_value(json!({"schema_version":1,"actions":[
        {"ordinal":0,"op":"command","request":{"company":0,"mode":"post","command":{"kind":"pause","mode":0,"paused":false}}},
        {"ordinal":1,"op":"tick","count":1}
    ]}))?;
    let result = run_replay(&world, &ReplayCursor::new(plan)?, None, &mut |_, _| Ok(()));
    assert!(result.is_err());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn resumed_cursor_revalidates_total_tick_budget_and_past_labels() -> Result {
    for actions in [
        json!([{"ordinal":0,"op":"tick","count":60_000},{"ordinal":1,"op":"tick","count":60_000}]),
        json!([{"ordinal":0,"op":"checkpoint","label":"same"},{"ordinal":1,"op":"checkpoint","label":"same"}]),
    ] {
        let cursor: ReplayCursor = serde_json::from_value(
            json!({"plan":{"schema_version":1,"actions":actions},"position":1,"next_ordinal":1}),
        )?;
        assert!(cursor.validate().is_err());
    }
    Ok(())
}

#[test]
fn rejects_checkpoint_names_that_collide_on_case_insensitive_filesystems() -> Result {
    for actions in [
        json!([{"ordinal":0,"op":"checkpoint","label":"Initial"}]),
        json!([{"ordinal":0,"op":"checkpoint","label":"Road"},{"ordinal":1,"op":"checkpoint","label":"road"}]),
    ] {
        let plan: ReplayPlan =
            serde_json::from_value(json!({"schema_version":1,"actions":actions}))?;
        assert!(ReplayCursor::new(plan).is_err());
    }
    Ok(())
}
