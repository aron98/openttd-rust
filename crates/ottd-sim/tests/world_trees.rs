//! Tree callbacks execute inside the complete saved-world transaction.
use ottd_save::{
    Savegame, TileRawParts, TileState, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::advance_world;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn field(chunk: [u8; 4], name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value: WireValue::Unsigned(value),
    }
}
fn tree_world(stage: u8, count: u8, setting: u64, density: u16) -> Result<World> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    // (x,y)=(10,9): 11*x+9*y = 191, so the initial callback is a growth boundary.
    let index = 9 * 64 + 10;
    let tile = world.map().tiles().get(index).ok_or("tile")?;
    let value = TileState::from(TileRawParts {
        tile_type: 0x40,
        height: tile.height(),
        m1: 0x70,
        m2: density << 4,
        m3: 0,
        m4: 0,
        m5: (count.checked_sub(1).ok_or("tree count")? << 6) | stage,
        m6: 0,
        m7: 0,
        m8: 0,
    });
    world.edit_batch(vec![
        WorldEdit::Tile {
            index: u32::try_from(index)?,
            value,
        },
        field(*b"DATE", "cur_tileloop_tile", u64::try_from(index)?),
        field(*b"PATS", "construction.extra_tree_placement", setting),
    ])?;
    Ok(world)
}
#[test]
fn dead_tree_replants_inside_saved_world_when_spread_disabled() -> Result {
    let mut world = tree_world(6, 4, 0, 1)?;
    advance_world(&mut world, 1)?;
    let tile = world.map().tiles().get(586).ok_or("tile")?;
    assert_eq!((tile.m5(), tile.m2(), tile.m1()), (192, 32, 0x70));
    Ok(())
}
#[test]
fn grass_grows_before_no_growth_setting_returns() -> Result {
    let mut world = tree_world(6, 2, 3, 1)?;
    advance_world(&mut world, 1)?;
    let tile = world.map().tiles().get(586).ok_or("tile")?;
    assert_eq!((tile.m5(), tile.m2()), (70, 32));
    Ok(())
}

#[test]
fn every_nonrandom_growth_stage_preserves_tree_count_and_saved_bits() -> Result {
    for (stage, next) in [(0, 1), (1, 2), (2, 3), (4, 5), (5, 6), (6, 0)] {
        for count in 1..=4 {
            let mut world = tree_world(stage, count, 0, 3)?;
            let before = world.saved_json()?;
            advance_world(&mut world, 1)?;
            let tile = world.map().tiles().get(586).ok_or("tile")?;
            assert_eq!(
                tile.m5(),
                (count.checked_sub(1).ok_or("tree count")? << 6) | next
            );
            let after = world.saved_json()?;
            for name in ["random_state[0]", "random_state[1]", "trees_tick_counter"] {
                let pointer = format!("/chunks/DATE/records/0/{name}");
                assert_eq!(
                    before.pointer(&pointer).ok_or("before field")?,
                    after.pointer(&pointer).ok_or("after field")?
                );
            }
        }
    }
    Ok(())
}

#[test]
fn grown_tree_draws_once_for_every_no_spread_branch() -> Result {
    for branch in 0..8 {
        let seed = (0..1024)
            .find(|word| ottd_core::Randomizer::from_state([*word, 0]).next_u32() & 7 == branch)
            .ok_or("seed")?;
        let mut world = tree_world(3, 4, 0, 3)?;
        world.edit_batch(vec![
            field(*b"DATE", "random_state[0]", u64::from(seed)),
            field(*b"DATE", "random_state[1]", 0),
        ])?;
        let mut expected = ottd_core::Randomizer::from_state([seed, 0]);
        let _draw = expected.next_u32();
        advance_world(&mut world, 1)?;
        let tile = world.map().tiles().get(586).ok_or("tile")?;
        assert_eq!(tile.m5(), 0xc0 | if branch == 0 { 4 } else { 3 });
        let saved = world.saved_json()?;
        for (name, value) in ["random_state[0]", "random_state[1]"]
            .into_iter()
            .zip(expected.state())
        {
            assert_eq!(
                saved
                    .pointer(&format!("/chunks/DATE/records/0/{name}"))
                    .and_then(serde_json::Value::as_u64),
                Some(u64::from(value))
            );
        }
    }
    Ok(())
}

#[test]
fn late_industry_failure_rolls_back_tree_and_disaster_rng_edits() -> Result {
    let mut world = tree_world(3, 2, 0, 1)?;
    // Synthetic industry refusal follows the first tree draw and the disaster reset.
    world.edit_batch(vec![
        field(*b"DATE", "date_fract", 72),
        field(*b"DATE", "economy_date_fract", 72),
        field(*b"DATE", "next_disaster_start", 1),
        field(*b"ECMY", "industry_daily_change_counter", 65_535),
        field(*b"PATS", "difficulty.industry_density", 1),
    ])?;
    let before = world.saved_json()?;
    let derived = world.derived().clone();
    let mut prefix = World::decode(&world.to_savegame()?)?;
    advance_world(&mut prefix, 1)?;
    let progressed = prefix.saved_json()?;
    for field in ["random_state[0]", "random_state[1]"] {
        let path = format!("/chunks/DATE/records/0/{field}");
        assert_ne!(progressed.pointer(&path), before.pointer(&path));
    }
    assert_ne!(prefix.map().tiles().get(586), world.map().tiles().get(586));
    let result = advance_world(&mut world, 2);
    assert!(matches!(
        result,
        Err(ottd_sim::WorldTickError::Unsupported {
            phase: "industry_day",
            reason
        }) if reason == "call 1: automatic industry creation settings"
    ));
    assert_eq!(world.saved_json()?, before);
    assert_eq!(world.derived(), &derived);
    Ok(())
}

#[test]
fn zero_and_paused_calls_preserve_tree_and_random_state() -> Result {
    for paused in [false, true] {
        let mut world = tree_world(3, 3, 0, 1)?;
        if paused {
            world.edit_batch(vec![field(*b"DATE", "pause_mode", 1)])?;
        }
        let before = world.saved_json()?;
        advance_world(&mut world, u32::from(paused))?;
        let after = world.saved_json()?;
        for key in [
            "DATE", "MAPT", "MAP1", "MAP2", "MAP3", "MAP4", "MAP5", "MAPE", "MAP7", "MAP8",
        ] {
            let pointer = format!("/chunks/{key}");
            assert_eq!(before.pointer(&pointer), after.pointer(&pointer));
        }
    }
    Ok(())
}

#[test]
fn tree_growth_save_reload_preserves_continuation() -> Result {
    let mut uninterrupted = tree_world(4, 2, 0, 0)?;
    let mut resumed = uninterrupted.clone();
    advance_world(&mut uninterrupted, 8193)?;
    advance_world(&mut resumed, 4096)?;
    let mut resumed = World::decode(&resumed.to_savegame()?)?;
    advance_world(&mut resumed, 4097)?;
    assert_eq!(uninterrupted.saved_json()?, resumed.saved_json()?);
    assert_eq!(uninterrupted.derived(), resumed.derived());
    assert_eq!(resumed.map().tiles().get(586).ok_or("tile")?.m5(), 64);
    Ok(())
}
