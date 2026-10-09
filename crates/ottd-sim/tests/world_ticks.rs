//! Saved-world loop admission and transactional state changes.
#![expect(
    clippy::indexing_slicing,
    reason = "immutable serde_json Value indexing returns Null for missing paths, making these assertions fail rather than panic"
)]
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::advance_world;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn edit(chunk: [u8; 4], id: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: id,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
#[test]
fn paused_populated_world_refills_limits_without_advancing_clock() -> Result {
    let mut world = world()?;
    world.edit_batch(vec![
        edit(*b"DATE", 0, "pause_mode", WireValue::Unsigned(1)),
        edit(*b"PLYR", 0, "clear_limit", WireValue::Unsigned(0)),
    ])?;
    let before = world.saved_json()?;
    advance_world(&mut world, 1)?;
    let after = world.saved_json()?;
    assert_eq!(before["chunks"]["DATE"], after["chunks"]["DATE"]);
    assert_eq!(
        after["chunks"]["PLYR"]["records"]["0"]["clear_limit"],
        before["chunks"]["PATS"]["records"]["0"]["construction.clear_per_64k_frames"]
    );
    Ok(())
}
#[test]
fn unsupported_unpaused_world_is_unchanged() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    assert!(advance_world(&mut world, 2).is_err());
    assert_eq!(before, world.saved_json()?);
    Ok(())
}

fn clear_world() -> Result<World> {
    let world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    Ok(world)
}
fn boundary(world: &mut World, month: u8, day: u8) -> Result {
    let date = ottd_core::CalendarDate::from_ymd(2100, month, day)?.raw();
    world.edit_batch(vec![
        edit(*b"DATE", 0, "date", WireValue::Signed(i64::from(date))),
        edit(
            *b"DATE",
            0,
            "economy_date",
            WireValue::Signed(i64::from(date)),
        ),
        edit(*b"DATE", 0, "date_fract", WireValue::Unsigned(73)),
        edit(*b"DATE", 0, "economy_date_fract", WireValue::Unsigned(73)),
    ])?;
    Ok(())
}
#[test]
fn clear_world_advances_saved_clock_and_global_counters() -> Result {
    let mut world = clear_world()?;
    advance_world(&mut world, 74)?;
    let saved = world.saved_json()?;
    assert_eq!(saved["chunks"]["DATE"]["records"]["0"]["tick_counter"], 74);
    assert_eq!(
        saved["chunks"]["DATE"]["records"]["0"]["next_disaster_start"],
        999
    );
    assert_eq!(
        saved["chunks"]["ECMY"]["records"]["0"]["industry_daily_change_counter"],
        132
    );
    Ok(())
}
#[test]
fn year_boundary_charges_month_before_expense_rollover() -> Result {
    let mut world = clear_world()?;
    boundary(&mut world, 11, 31)?;
    advance_world(&mut world, 1)?;
    let saved = world.saved_json()?;
    let company = &saved["chunks"]["PLYR"]["records"]["0"];
    assert_eq!(company["money"], 999_809);
    assert_eq!(company["yearly_expenses"][11], 0);
    assert_eq!(company["yearly_expenses"][24], 166);
    assert_eq!(company["yearly_expenses"][25], 25);
    assert_eq!(company["old_economy"].as_array().ok_or("history")?.len(), 1);
    assert_eq!(company["old_economy"][0]["company_value"], 900_000);
    assert_eq!(company["cur_economy"][0]["expenses"], -166);
    Ok(())
}
#[test]
fn later_disaster_expiry_rolls_back_earlier_ticks() -> Result {
    let mut world = clear_world()?;
    boundary(&mut world, 0, 1)?;
    world.edit_batch(vec![edit(
        *b"DATE",
        0,
        "next_disaster_start",
        WireValue::Unsigned(2),
    )])?;
    let before = world.saved_json()?;
    let result = advance_world(&mut world, 75);
    assert!(matches!(
        result,
        Err(ottd_sim::WorldTickError::Unsupported {
            phase: "disaster_day",
            ..
        })
    ));
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn saved_reload_continuation_matches_uninterrupted_ticks() -> Result {
    let mut whole = clear_world()?;
    let mut split = whole.clone();
    advance_world(&mut whole, 256)?;
    advance_world(&mut split, 128)?;
    let mut reloaded = World::decode(&split.to_savegame()?)?;
    advance_world(&mut reloaded, 128)?;
    assert_eq!(whole.saved_json()?, reloaded.saved_json()?);
    assert_eq!(whole.derived(), reloaded.derived());
    Ok(())
}

#[test]
fn industry_attempt_horizon_rejects_without_publishing_clock_changes() -> Result {
    let mut world = clear_world()?;
    boundary(&mut world, 0, 1)?;
    world.edit_batch(vec![edit(
        *b"ECMY",
        0,
        "industry_daily_change_counter",
        WireValue::Unsigned(65_535),
    )])?;
    let before = world.saved_json()?;
    assert!(matches!(
        advance_world(&mut world, 1),
        Err(ottd_sim::WorldTickError::Unsupported {
            phase: "industry_day",
            ..
        })
    ));
    assert_eq!(before, world.saved_json()?);
    Ok(())
}

#[test]
fn competitor_timer_expiry_rolls_back_complete_request() -> Result {
    let mut world = clear_world()?;
    world.edit_batch(vec![edit(
        *b"DATE",
        0,
        "competitors_interval",
        WireValue::Unsigned(2),
    )])?;
    let before = world.saved_json()?;
    assert!(matches!(
        advance_world(&mut world, 2),
        Err(ottd_sim::WorldTickError::Unsupported {
            phase: "tick_timer",
            ..
        })
    ));
    assert_eq!(before, world.saved_json()?);
    Ok(())
}

#[test]
fn arbitrary_saved_wallclock_settings_freeze_calendar_only() -> Result {
    let mut world = clear_world()?;
    let date = i64::from(2100_i32.saturating_mul(360));
    world.edit_batch(vec![
        edit(
            *b"PATS",
            0,
            "economy.timekeeping_units",
            WireValue::Unsigned(1),
        ),
        edit(
            *b"PATS",
            0,
            "economy.minutes_per_calendar_year",
            WireValue::Unsigned(0),
        ),
        edit(*b"DATE", 0, "economy_date", WireValue::Signed(date)),
    ])?;
    let before = world.saved_json()?;
    advance_world(&mut world, 74)?;
    let after = world.saved_json()?;
    assert_eq!(
        after["chunks"]["DATE"]["records"]["0"]["date"],
        before["chunks"]["DATE"]["records"]["0"]["date"]
    );
    assert_eq!(after["chunks"]["DATE"]["records"]["0"]["date_fract"], 0);
    assert_eq!(
        after["chunks"]["DATE"]["records"]["0"]["economy_date"],
        date.saturating_add(1)
    );
    Ok(())
}
