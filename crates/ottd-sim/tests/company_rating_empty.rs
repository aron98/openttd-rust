//! Synthetic boundary controls for the real empty Runtime adapter, not native fixtures.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::runtime::SimulationRuntime;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn field(chunk: [u8; 4], name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
fn boundary() -> Result<World> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let date = i64::from(ottd_core::CalendarDate::from_ymd(2100, 2, 31)?.raw());
    world.edit_batch(vec![
        field(*b"DATE", "date", WireValue::Signed(date)),
        field(*b"DATE", "economy_date", WireValue::Signed(date)),
        field(*b"DATE", "date_fract", WireValue::Unsigned(73)),
        field(*b"DATE", "economy_date_fract", WireValue::Unsigned(73)),
    ])?;
    Ok(world)
}
#[test]
fn empty_runtime_publishes_raw_rating_while_world_only_matches_saved_effects() -> Result {
    let mut world_only = boundary()?;
    let mut runtime = SimulationRuntime::restore_vanilla(world_only.clone())?;
    assert_eq!(runtime.company_rating_caches().parts(), &[[0; 10]; 15]);
    runtime.advance_world(1)?;
    ottd_sim::advance_world(&mut world_only, 1)?;
    assert_eq!(runtime.saved_json()?, world_only.saved_json()?);
    assert_eq!(
        runtime.company_rating_caches().parts().first(),
        Some(&[0, 0, 0, 0, 0, 0, 0, 1_000_000, 150_000, 35])
    );
    assert!(
        runtime
            .company_rating_caches()
            .parts()
            .iter()
            .skip(1)
            .all(|parts| *parts == [0; 10])
    );
    Ok(())
}
#[test]
fn zero_company_runtime_advances_without_fabricating_owner_slots() -> Result {
    let mut world = boundary()?;
    world.edit_batch(vec![WorldEdit::RemoveRecord {
        chunk: *b"PLYR",
        record: 0,
    }])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.saved_json()?;
    runtime.advance_world(1)?;
    assert_ne!(runtime.saved_json()?, before);
    assert_eq!(runtime.company_rating_caches().parts(), &[[0; 10]; 15]);
    Ok(())
}
#[test]
fn quarter_decrements_only_nonzero_preview_counter() -> Result {
    for preview in [0_u64, 1, 255] {
        let mut world = boundary()?;
        world.edit_batch(vec![field(
            *b"PLYR",
            "block_preview",
            WireValue::Unsigned(preview),
        )])?;
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        runtime.advance_world(1)?;
        assert_eq!(
            runtime
                .saved_json()?
                .pointer("/chunks/PLYR/records/0/block_preview")
                .and_then(serde_json::Value::as_u64),
            Some(preview.saturating_sub(1))
        );
    }
    Ok(())
}
#[test]
fn raw_loan_headroom_stays_negative_while_weighted_contribution_clamps() -> Result {
    let mut world = boundary()?;
    world.edit_batch(vec![field(
        *b"PLYR",
        "current_loan",
        WireValue::Signed(300_000),
    )])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    runtime.advance_world(1)?;
    assert_eq!(
        runtime.company_rating_caches().parts().first(),
        Some(&[0, 0, 0, 0, 0, 0, 0, 1_000_000, -50_000, 5])
    );
    Ok(())
}
