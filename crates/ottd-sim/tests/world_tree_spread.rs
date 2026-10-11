//! Original-saved fixtures exercise spreading through the complete transaction.
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{WorldTickError, advance_world};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
const RAINFOREST: &[u8] = include_bytes!("../../../fixtures/tree-spread/rainforest-only.sav");
const SPREAD: &[u8] = include_bytes!("../../../fixtures/tree-spread/all-spread.sav");
const BEFORE: &[u8] = include_bytes!("../../../fixtures/tree-spread/before-placement.sav");
const AFTER: &[u8] = include_bytes!("../../../fixtures/tree-spread/after-placement.sav");
fn world(bytes: &[u8]) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        bytes,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn date(name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"DATE",
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value: WireValue::Unsigned(value),
    }
}
#[test]
fn both_spreading_settings_match_complete_original_saved_worlds() -> Result {
    for (input, expected) in [
        (
            RAINFOREST,
            include_bytes!("../../../fixtures/tree-spread/rainforest-only-4096.sav").as_slice(),
        ),
        (
            SPREAD,
            include_bytes!("../../../fixtures/tree-spread/all-spread-4096.sav").as_slice(),
        ),
    ] {
        let mut actual = world(input)?;
        advance_world(&mut actual, 4096)?;
        let expected = world(expected)?;
        assert_eq!(actual.saved_json()?, expected.saved_json()?);
        assert_eq!(actual.derived(), expected.derived());
    }
    Ok(())
}
#[test]
fn genuine_counter_wrap_places_a_tree_and_survives_reload() -> Result {
    let initial = world(BEFORE)?;
    assert_eq!(
        initial.map().tiles().get(2469).ok_or("tile")?.tile_type() >> 4,
        0
    );
    let mut actual = world(BEFORE)?;
    advance_world(&mut actual, 1)?;
    let tree = actual.map().tiles().get(2469).ok_or("tile")?;
    assert_eq!((tree.tile_type() >> 4, tree.m3(), tree.m5()), (4, 4, 0));
    assert_eq!(actual.saved_json()?, world(AFTER)?.saved_json()?);
    let mut resumed = World::decode(&actual.to_savegame()?)?;
    advance_world(&mut actual, 128)?;
    advance_world(&mut resumed, 128)?;
    assert_eq!(actual.saved_json()?, resumed.saved_json()?);
    assert_eq!(actual.derived(), resumed.derived());
    Ok(())
}
#[test]
fn failure_after_natural_placement_rolls_back_whole_world() -> Result {
    let mut actual = world(BEFORE)?;
    actual.edit_batch(vec![date("competitors_interval", 2)])?;
    let before = actual.saved_json()?;
    let derived = actual.derived().clone();
    let failure = advance_world(&mut actual, 2);
    assert!(matches!(
        failure,
        Err(WorldTickError::Unsupported {
            phase: "tick_timer",
            ..
        })
    ));
    assert_eq!(actual.saved_json()?, before);
    assert_eq!(actual.derived(), &derived);
    Ok(())
}
#[test]
fn spreading_zero_and_paused_calls_preserve_tree_rng_and_counter() -> Result {
    for input in [RAINFOREST, SPREAD, BEFORE] {
        for paused in [false, true] {
            let mut actual = world(input)?;
            if paused {
                actual.edit_batch(vec![date("pause_mode", 1)])?;
            }
            let before = actual.saved_json()?;
            let derived = actual.derived().clone();
            advance_world(&mut actual, if paused { 32 } else { 0 })?;
            let after = actual.saved_json()?;
            if paused {
                for chunk in [
                    "DATE", "MAPT", "MAP1", "MAP2", "MAP3", "MAP4", "MAP5", "MAPE", "MAP7", "MAP8",
                ] {
                    assert_eq!(
                        after.pointer(&format!("/chunks/{chunk}")),
                        before.pointer(&format!("/chunks/{chunk}"))
                    );
                }
            } else {
                assert_eq!(after, before);
            }
            assert_eq!(actual.derived(), &derived);
        }
    }
    Ok(())
}

#[test]
fn original_asserted_neighbor_boundary_refuses_without_publication() -> Result {
    let mut actual = world(include_bytes!(
        "../../../fixtures/tree-spread/edge-planted.sav"
    ))?;
    let before = actual.saved_json()?;
    let derived = actual.derived().clone();
    let failure = advance_world(&mut actual, 16384);
    match failure {
        Err(WorldTickError::Unsupported {
            phase: "trees",
            reason,
        }) => {
            assert_eq!(
                reason,
                "call 12317: native neighbor tile assertion boundary"
            );
        }
        other => {
            return Err(format!("expected original asserted tree boundary, got {other:?}").into());
        }
    }
    assert_eq!(actual.saved_json()?, before);
    assert_eq!(actual.derived(), &derived);
    Ok(())
}
