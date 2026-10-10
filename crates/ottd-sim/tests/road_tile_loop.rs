//! Original-produced empty-road state-loop witnesses; no vehicle movement claim.
use ottd_save::{Savegame, world::World};
use ottd_sim::advance_world;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn built() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/road-tile/built.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
#[test]
fn native_first_road_visit_matches_complete_world() -> Result {
    let mut world = built()?;
    advance_world(&mut world, 9)?;
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/road-tile/after-road.world.json"
    ))?;
    assert_eq!(world.saved_json()?, expected);
    let derived: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/road-tile/after-road.derived.json"
    ))?;
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    Ok(())
}

#[test]
fn native_complete_cycles_and_reloaded_visits_match() -> Result {
    let cases: [(&[u8], u32, &str, &str); 5] = [
        (
            include_bytes!("../../../fixtures/road-tile/built.sav"),
            149,
            include_str!("../../../fixtures/road-tile/after-depot.world.json"),
            include_str!("../../../fixtures/road-tile/after-depot.derived.json"),
        ),
        (
            include_bytes!("../../../fixtures/road-tile/built.sav"),
            256,
            include_str!("../../../fixtures/road-tile/cycle-one.world.json"),
            include_str!("../../../fixtures/road-tile/cycle-one.derived.json"),
        ),
        (
            include_bytes!("../../../fixtures/road-tile/built.sav"),
            512,
            include_str!("../../../fixtures/road-tile/cycle-two.world.json"),
            include_str!("../../../fixtures/road-tile/cycle-two.derived.json"),
        ),
        (
            include_bytes!("../../../fixtures/road-tile/before-road.sav"),
            1,
            include_str!("../../../fixtures/road-tile/after-road.world.json"),
            include_str!("../../../fixtures/road-tile/after-road.derived.json"),
        ),
        (
            include_bytes!("../../../fixtures/road-tile/before-depot.sav"),
            1,
            include_str!("../../../fixtures/road-tile/after-depot.world.json"),
            include_str!("../../../fixtures/road-tile/after-depot.derived.json"),
        ),
    ];
    for (input, ticks, saved, derived) in cases {
        let mut world = World::decode(&Savegame::decode(input, ottd_save::DEFAULT_MAX_BYTES)?)?;
        advance_world(&mut world, ticks)?;
        assert_eq!(
            world.saved_json()?,
            serde_json::from_str::<serde_json::Value>(saved)?
        );
        assert_eq!(
            serde_json::to_value(world.derived())?,
            serde_json::from_str::<serde_json::Value>(derived)?
        );
    }
    Ok(())
}

#[test]
fn unsupported_road_fields_are_rejected_before_publication() -> Result {
    for case in 0..8 {
        let mut world = built()?;
        let mut tile = ottd_save::TileRawParts::from(world.map().tiles().get(648).ok_or("tile")?);
        match case {
            0 => tile.m6 = (tile.m6 & !0x38) | 0x30,
            1 => tile.m6 = (tile.m6 & !0x38) | 16,
            2 => tile.m4 = 1,
            3 => tile.m8 = 64,
            4 => tile.tile_type |= 4,
            5 => tile.height = 3,
            6 => tile.m7 |= 32,
            7 => tile.m2 = u16::MAX,
            _ => return Err("unknown refusal case".into()),
        }
        world.edit_tile(648, &tile.into())?;
        let before = world.saved_json()?;
        let derived = serde_json::to_value(world.derived())?;
        assert!(advance_world(&mut world, 512).is_err(), "case {case}");
        assert_eq!(world.saved_json()?, before, "case {case}");
        assert_eq!(
            serde_json::to_value(world.derived())?,
            derived,
            "case {case}"
        );
    }
    Ok(())
}

#[test]
fn road_context_settings_refuse_before_publication() -> Result {
    use ottd_save::{WireValue, world::PathElement};
    for (chunk, name, value) in [
        (*b"CITY", "road_build_months", 1),
        (*b"CITY", "fund_buildings_months", 1),
        (*b"PATS", "game_creation.landscape", 1),
    ] {
        let mut world = built()?;
        world.edit_field(
            chunk,
            0,
            &[PathElement::Field(name.into())],
            WireValue::Unsigned(value),
        )?;
        let before = world.saved_json()?;
        assert!(advance_world(&mut world, 256).is_err());
        assert_eq!(world.saved_json()?, before);
    }
    Ok(())
}

#[test]
fn qualified_roads_preserve_map_only_and_zero_tick_contracts() -> Result {
    let mut world = built()?;
    let before = world.saved_json()?;
    advance_world(&mut world, 0)?;
    assert_eq!(world.saved_json()?, before);
    let tile = ottd_sim::Tile {
        tile_type: 0x20,
        height: 4,
        m1: 0,
        m2: 0,
        m3: 0,
        m4: 0,
        m5: 10,
        m6: 0,
        m7: 0,
        m8: 4032,
    };
    assert!(
        ottd_sim::Landscape::new(
            ottd_sim::Map {
                width: 64,
                height: 64,
                tiles: vec![tile; 4096]
            },
            1
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn road_visit_masks_only_roadside_bits() -> Result {
    let mut world = built()?;
    let mut parts = ottd_save::TileRawParts::from(world.map().tiles().get(648).ok_or("tile")?);
    parts.m6 = 0xc7;
    world.edit_tile(648, &parts.into())?;
    advance_world(&mut world, 55)?;
    parts.m6 = 0xcf;
    assert_eq!(*world.map().tiles().get(648).ok_or("tile")?, parts.into());
    Ok(())
}
