//! Original saves verify the complete forest consumer; injected edge cases are labelled.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{WorldTickError, advance_world, runtime::SimulationRuntime};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
const START: &[u8] = include_bytes!("../../../fixtures/industry-forest/construction.sav");
const END: &[u8] = include_bytes!("../../../fixtures/industry-forest/after-8192.sav");
fn world(bytes: &[u8]) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        bytes,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
const fn edit(chunk: [u8; 4], path: Vec<PathElement>, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: 0,
        path,
        value: WireValue::Unsigned(value),
    }
}
fn field(name: &str) -> PathElement {
    PathElement::Field(name.into())
}
fn amount(world: &World, path: &str) -> Result<u64> {
    world
        .saved_json()?
        .pointer(path)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "amount".into())
}

#[test]
fn original_forest_construction_stockpiles_and_months_match_complete_world() -> Result {
    let mut runtime = SimulationRuntime::restore_vanilla(world(START)?)?;
    let cache = runtime.industry_caches().clone();
    runtime.advance_world(8192)?;
    let expected = world(END)?;
    assert_eq!(runtime.saved_json()?, expected.saved_json()?);
    assert_eq!(runtime.world().derived(), expected.derived());
    assert_eq!(runtime.industry_caches(), &cache);
    assert_eq!(cache.type_members.get(3), Some(&vec![0]));
    assert_eq!(cache.daily_increment, 132);
    Ok(())
}

#[test]
fn stockpile_production_runs_while_all_forest_tiles_are_under_construction() -> Result {
    let mut world = world(START)?;
    advance_world(&mut world, 256)?;
    assert_eq!(
        amount(&world, "/chunks/INDY/records/0/produced/0/waiting")?,
        13
    );
    let tiles: Vec<_> = world
        .map()
        .tiles()
        .iter()
        .filter(|t| t.tile_type() >> 4 == 8)
        .collect();
    assert_eq!(tiles.len(), 23);
    assert!(tiles.iter().all(|t| t.m1() == 0x64));
    Ok(())
}

#[test]
fn injected_stockpile_threshold_is_strict_and_drain_is_byte_clamped() -> Result {
    for (waiting, remaining, produced) in [(30, 30, 0), (31, 0, 31), (300, 45, 255)] {
        let mut world = world(END)?;
        world.edit_batch(vec![
            edit(*b"DATE", vec![field("cur_tileloop_tile")], 520),
            edit(*b"INDY", vec![field("counter")], 17),
            edit(
                *b"INDY",
                vec![field("produced"), PathElement::Index(0), field("waiting")],
                waiting,
            ),
            edit(
                *b"INDY",
                vec![
                    field("produced"),
                    PathElement::Index(0),
                    field("history"),
                    PathElement::Index(0),
                    field("production"),
                ],
                0,
            ),
        ])?;
        advance_world(&mut world, 1)?;
        assert_eq!(
            amount(&world, "/chunks/INDY/records/0/produced/0/waiting")?,
            remaining
        );
        assert_eq!(
            amount(
                &world,
                "/chunks/INDY/records/0/produced/0/history/0/production"
            )?,
            produced
        );
    }
    Ok(())
}

#[test]
fn injected_late_disaster_refusal_discards_prior_industry_counter_and_random() -> Result {
    let mut world = world(END)?;
    world.edit_batch(vec![
        edit(*b"INDY", vec![field("counter")], 64),
        edit(*b"DATE", vec![field("next_disaster_start")], 1),
        edit(*b"DATE", vec![field("date_fract")], 72),
        edit(*b"DATE", vec![field("economy_date_fract")], 72),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.saved_json()?;
    let derived = runtime.world().derived().clone();
    let towns = runtime.town_house_caches().clone();
    let industries = runtime.industry_caches().clone();
    let mut prefix = SimulationRuntime::restore_vanilla(World::decode(&runtime.to_savegame()?)?)?;
    prefix.advance_world(1)?;
    assert_eq!(
        amount(prefix.world(), "/chunks/INDY/records/0/counter")?,
        63
    );
    assert_ne!(
        prefix
            .saved_json()?
            .pointer("/chunks/DATE/records/0/random_state[0]"),
        before.pointer("/chunks/DATE/records/0/random_state[0]")
    );
    assert!(matches!(
        runtime.advance_world(2),
        Err(WorldTickError::Unsupported {
            phase: "disaster_day",
            ..
        })
    ));
    assert_eq!(runtime.saved_json()?, before);
    assert_eq!(runtime.world().derived(), &derived);
    assert_eq!(runtime.town_house_caches(), &towns);
    assert_eq!(runtime.industry_caches(), &industries);
    Ok(())
}
