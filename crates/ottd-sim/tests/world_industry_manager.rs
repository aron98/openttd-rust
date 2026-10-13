//! Explicit unit states for manager bookkeeping; these are not native parity fixtures.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{WorldTickError, advance_world};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn edit(chunk: [u8; 4], record: u32, field: &str, value: u32) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path: vec![PathElement::Field(field.into())],
        value: WireValue::Unsigned(u64::from(value)),
    }
}
fn number(world: &serde_json::Value, chunk: &str, record: u32, field: &str) -> Result<u64> {
    world
        .pointer(&format!("/chunks/{chunk}/records/{record}/{field}"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "number missing".into())
}
fn boundary() -> Result<World> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    world.edit_batch(vec![
        edit(*b"DATE", 0, "date_fract", 73),
        edit(*b"DATE", 0, "economy_date_fract", 73),
        edit(*b"DATE", 0, "random_state[0]", 3_574_084_369),
        edit(*b"DATE", 0, "random_state[1]", 3_889_121_767),
        edit(*b"ECMY", 0, "industry_daily_change_counter", 65_472),
        edit(*b"PATS", 0, "difficulty.industry_density", 0),
    ])?;
    Ok(world)
}

#[test]
fn fund_only_recomputes_every_type_and_decrements_waits_preserving_maximums() -> Result {
    let mut world = boundary()?;
    let mut edits = Vec::new();
    for id in 0..240 {
        for (field, value) in [
            ("probability", 100 + id),
            ("min_number", 1),
            ("target_count", 2),
            ("max_wait", 100 + id),
            ("wait_count", id),
        ] {
            edits.push(edit(*b"ITBL", id, field, value));
        }
    }
    world.edit_batch(edits)?;
    advance_world(&mut world, 1)?;
    let saved = world.saved_json()?;
    for id in 0..240 {
        for field in ["probability", "min_number", "target_count"] {
            assert_eq!(number(&saved, "ITBL", id, field)?, 0);
        }
        assert_eq!(
            number(&saved, "ITBL", id, "wait_count")?,
            u64::from(id.saturating_sub(1))
        );
        assert_eq!(number(&saved, "ITBL", id, "max_wait")?, u64::from(100 + id));
    }
    assert_eq!(
        number(&saved, "ECMY", 0, "industry_daily_change_counter")?,
        68
    );
    let mut random = ottd_core::Randomizer::from_state([3_574_084_369, 3_889_121_767]);
    let _ = random.next_u32();
    for (field, expected) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(random.state())
    {
        assert_eq!(number(&saved, "DATE", 0, field)?, u64::from(expected));
    }
    Ok(())
}

#[test]
fn unchanged_plan_retains_targets_and_waiting_types_do_not_create() -> Result {
    let mut world = boundary()?;
    world.edit_batch(vec![
        edit(*b"IBLD", 0, "wanted_inds", 2 << 16),
        edit(*b"ITBL", 0, "target_count", 1),
        edit(*b"ITBL", 239, "target_count", 1),
        edit(*b"ITBL", 0, "wait_count", 2),
        edit(*b"ITBL", 239, "wait_count", 1),
    ])?;
    advance_world(&mut world, 1)?;
    let saved = world.saved_json()?;
    for id in [0, 239] {
        assert_eq!(number(&saved, "ITBL", id, "target_count")?, 1);
    }
    assert_eq!(number(&saved, "ITBL", 0, "wait_count")?, 1);
    assert_eq!(number(&saved, "ITBL", 239, "wait_count")?, 0);
    Ok(())
}

#[test]
fn reached_saved_creation_target_refuses_entire_transaction() -> Result {
    let mut world = boundary()?;
    world.edit_batch(vec![
        edit(*b"IBLD", 0, "wanted_inds", 1 << 16),
        edit(*b"ITBL", 239, "target_count", 1),
    ])?;
    let before = world.saved_json()?;
    let derived = world.derived().clone();
    assert!(
        matches!(advance_world(&mut world,1),Err(WorldTickError::Unsupported { phase:"industry_builder",reason }) if reason.contains("PlaceIndustry"))
    );
    assert_eq!(world.saved_json()?, before);
    assert_eq!(world.derived(), &derived);
    Ok(())
}

#[test]
fn uint32_counter_wrap_has_no_attempt_and_no_random_draw() -> Result {
    let mut world = boundary()?;
    world.edit_batch(vec![edit(
        *b"ECMY",
        0,
        "industry_daily_change_counter",
        u32::MAX,
    )])?;
    let before = world.saved_json()?;
    advance_world(&mut world, 1)?;
    let saved = world.saved_json()?;
    assert_eq!(
        number(&saved, "ECMY", 0, "industry_daily_change_counter")?,
        131
    );
    for field in ["random_state[0]", "random_state[1]"] {
        assert_eq!(
            number(&saved, "DATE", 0, field)?,
            number(&before, "DATE", 0, field)?
        );
    }
    Ok(())
}

#[test]
fn multiple_empty_pool_attempts_consume_exactly_the_build_chance_draws() -> Result {
    let mut world = boundary()?;
    world.edit_batch(vec![edit(
        *b"ECMY",
        0,
        "industry_daily_change_counter",
        3 << 16,
    )])?;
    advance_world(&mut world, 1)?;
    let saved = world.saved_json()?;
    assert_eq!(
        number(&saved, "ECMY", 0, "industry_daily_change_counter")?,
        132
    );
    let mut random = ottd_core::Randomizer::from_state([3_574_084_369, 3_889_121_767]);
    for _ in 0..3 {
        let _ = random.next_u32();
    }
    for (field, expected) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(random.state())
    {
        assert_eq!(number(&saved, "DATE", 0, field)?, u64::from(expected));
    }
    Ok(())
}
