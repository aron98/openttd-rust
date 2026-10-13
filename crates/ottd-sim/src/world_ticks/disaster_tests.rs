//! Synthetic exact phase arithmetic/order; no original save or RNG injection claim.
use super::{State, WorldTickError, daily};
use crate::runtime::IndustryCaches;
use ottd_save::{Savegame, world::World};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
#[test]
fn disabled_delay_wraps_and_expiry_draws_shared_random_exactly_once() -> Result {
    for old in [0_i64, 1, 2, 65_535] {
        let mut world = world()?;
        let industries = IndustryCaches::restore(&world)?;
        let mut state = State::load(&mut world, None, industries);
        state.set_number(b"DATE", 0, "next_disaster_start", old)?;
        state.set_number(b"ECMY", 0, "industry_daily_change_counter", 0)?;
        let mut expected = ottd_core::Randomizer::from_state(state.random_state()?);
        let delay = if old == 1 {
            i64::from((expected.next_u32() & 511) + 730)
        } else {
            old.wrapping_sub(1) & 0xffff
        };
        daily(&mut state)?;
        assert_eq!(state.number(b"DATE", 0, "next_disaster_start")?, delay);
        assert_eq!(state.random_state()?, expected.state());
    }
    Ok(())
}
#[test]
fn reset_precedes_real_industry_callback_and_failed_transaction_discards_both() -> Result {
    let mut world = world()?;
    let before = world.saved_json()?;
    {
        let industries = IndustryCaches::restore(&world)?;
        let mut state = State::load(&mut world, None, industries);
        state.set_number(b"DATE", 0, "next_disaster_start", 1)?;
        state.set_number(b"ECMY", 0, "industry_daily_change_counter", 65_535)?;
        state.set_number(b"PATS", 0, "difficulty.industry_density", 1)?;
        let mut expected = ottd_core::Randomizer::from_state(state.random_state()?);
        let delay = i64::from((expected.next_u32() & 511) + 730);
        assert!(
            matches!(daily(&mut state), Err(WorldTickError::Unsupported {
            phase: "industry_day", reason
        }) if reason == "automatic industry creation settings")
        );
        assert_eq!(state.number(b"DATE", 0, "next_disaster_start")?, delay);
        assert_eq!(state.random_state()?, expected.state());
    }
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn enabled_disasters_remain_an_unchanged_domain_refusal() -> Result {
    let mut world = world()?;
    world.edit_batch(vec![ottd_save::world::WorldEdit::Field {
        chunk: *b"PATS",
        record: 0,
        path: vec![ottd_save::world::PathElement::Field(
            "difficulty.disasters".into(),
        )],
        value: ottd_save::WireValue::Signed(1),
    }])?;
    let before = world.saved_json()?;
    assert!(
        matches!(crate::advance_world(&mut world, 1), Err(WorldTickError::Unsupported {
        phase: "settings", reason
    }) if reason == "difficulty.disasters")
    );
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
