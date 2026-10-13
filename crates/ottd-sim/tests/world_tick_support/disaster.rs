//! Synthetic late industry rejection after a supported disaster reset.
use super::{Result, WireValue, advance_world, boundary, clear_world, edit};
#[test]
fn later_industry_refusal_after_disaster_reset_rolls_back_earlier_ticks() -> Result {
    let mut world = clear_world()?;
    boundary(&mut world, 0, 1)?;
    world.edit_batch(vec![edit(
        *b"DATE",
        0,
        "next_disaster_start",
        WireValue::Unsigned(2),
    )])?;
    let increment = ottd_sim::runtime::SimulationRuntime::restore_vanilla(world.clone())?
        .industry_caches()
        .daily_increment;
    world.edit_batch(vec![
        edit(
            *b"ECMY",
            0,
            "industry_daily_change_counter",
            WireValue::Unsigned(u64::from(65_535_u32.saturating_sub(increment))),
        ),
        edit(
            *b"PATS",
            0,
            "difficulty.industry_density",
            WireValue::Unsigned(1),
        ),
    ])?;
    let before = world.saved_json()?;
    let mut prefix = world.clone();
    advance_world(&mut prefix, 74)?;
    assert_ne!(prefix.saved_json()?, before);
    let result = advance_world(&mut world, 75);
    assert!(matches!(
        result,
        Err(ottd_sim::WorldTickError::Unsupported {
            phase: "industry_day",
            reason
        }) if reason == "call 74: automatic industry creation settings"
    ));
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
