//! Saved-world geometry and passive native map observations.
use ottd_core::terrain::GeometryError;
use ottd_save::{
    Savegame, TileRawParts, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::terrain::tile_slope_z;
use serde_json::Value;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn height_edit(world: &World, tile: u32, height: u8) -> Result<WorldEdit> {
    let mut parts = TileRawParts::from(
        world
            .map()
            .tiles()
            .get(usize::try_from(tile)?)
            .ok_or("tile")?,
    );
    parts.height = height;
    Ok(WorldEdit::Tile {
        index: tile,
        value: parts.into(),
    })
}
fn sloped_world() -> Result<World> {
    let mut world = world()?;
    let width = world.map().width();
    let mut edits = Vec::new();
    for y in 18_u32..=22 {
        for x in 19_u32..=23 {
            let distance = x
                .abs_diff(21)
                .checked_add(y.abs_diff(20))
                .ok_or("distance")?;
            let height = 4_u8
                .checked_add(u8::try_from(2_u32.saturating_sub(distance))?)
                .ok_or("height")?;
            let tile = y
                .checked_mul(width)
                .and_then(|n| n.checked_add(x))
                .ok_or("tile")?;
            edits.push(height_edit(&world, tile, height)?);
        }
    }
    for slope in 0_u8..15 {
        let x = 4_u32
            .checked_add(u32::from(slope % 5).checked_mul(8).ok_or("x")?)
            .ok_or("x")?;
        let y = 32_u32
            .checked_add(u32::from(slope / 5).checked_mul(8).ok_or("y")?)
            .ok_or("y")?;
        for (dx, dy, bit) in [(0, 0, 8), (1, 0, 1), (0, 1, 4), (1, 1, 2)] {
            let tile = y
                .checked_add(dy)
                .and_then(|n| n.checked_mul(width))
                .and_then(|n| n.checked_add(x))
                .and_then(|n| n.checked_add(dx))
                .ok_or("tile")?;
            edits.push(height_edit(
                &world,
                tile,
                4_u8.checked_add(u8::from(slope & bit != 0))
                    .ok_or("height")?,
            )?);
        }
    }
    world.edit_batch(edits)?;
    Ok(world)
}

#[test]
fn saved_corner_geometry_matches_native_orientation() -> Result {
    let world = sloped_world()?;
    let tile = world
        .map()
        .width()
        .checked_mul(20)
        .and_then(|n| n.checked_add(20))
        .ok_or("tile")?;
    let (slope, height) = tile_slope_z(&world, tile)?;
    assert_eq!((slope.raw(), height), (27, 4));
    assert_eq!(tile_slope_z(&world, u32::MAX), Err(GeometryError::Tile));
    Ok(())
}

#[test]
fn southeast_border_clamps_all_corners_to_last_tile() -> Result {
    let mut world = world()?;
    let last = u32::try_from(world.map().tiles().len())?
        .checked_sub(1)
        .ok_or("empty map")?;
    world.edit_batch(vec![height_edit(&world, last, 5)?])?;
    let (slope, height) = tile_slope_z(&world, last)?;
    assert_eq!((slope.raw(), height), (0, 5));
    Ok(())
}

#[test]
fn invalid_saved_corner_discontinuity_is_rejected() -> Result {
    let mut world = world()?;
    let tile = world
        .map()
        .width()
        .checked_mul(20)
        .and_then(|n| n.checked_add(20))
        .ok_or("tile")?;
    world.edit_batch(vec![height_edit(&world, tile, 10)?])?;
    assert_eq!(tile_slope_z(&world, tile), Err(GeometryError::Heights));
    Ok(())
}

#[test]
#[ignore = "fixture preparation for scripts/check-terrain.sh"]
fn prepare_native_terrain_worlds() -> Result {
    let directory = std::path::PathBuf::from(
        std::env::var_os("OTTD_TERRAIN_FIXTURE_DIR").ok_or("fixture directory")?,
    );
    std::fs::create_dir_all(&directory)?;
    for freeform in [false, true] {
        let mut world = sloped_world()?;
        let last = u32::try_from(world.map().tiles().len())?
            .checked_sub(1)
            .ok_or("empty map")?;
        world.edit_batch(vec![height_edit(&world, last, 5)?])?;
        world.edit_field(
            *b"PATS",
            0,
            &[PathElement::Field("construction.freeform_edges".into())],
            WireValue::Signed(i64::from(freeform)),
        )?;
        let bytes = world.to_savegame()?.encode(ottd_save::Compression::None)?;
        std::fs::write(
            directory.join(format!("freeform-{}.sav", u8::from(freeform))),
            bytes,
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "requires fresh native vectors; scripts/check-terrain.sh"]
fn world_geometry_matches_native_vectors() -> Result {
    let vectors: Value = serde_json::from_slice(&std::fs::read(
        std::env::var_os("OTTD_TERRAIN_JSON").ok_or("vectors")?,
    )?)?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(std::env::var_os("OTTD_TERRAIN_SAVE").ok_or("save")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let map = vectors.get("map").ok_or("map")?;
    assert_eq!(map.get("width"), Some(&Value::from(world.map().width())));
    assert_eq!(map.get("height"), Some(&Value::from(world.map().height())));
    let tiles = map.get("tiles").and_then(Value::as_array).ok_or("tiles")?;
    assert_eq!(tiles.len(), world.map().tiles().len());
    let mut seen = std::collections::BTreeSet::new();
    let mut shapes = std::collections::BTreeSet::new();
    for row in tiles {
        let tile = u32::try_from(row["tile"].as_u64().ok_or("tile")?)?;
        assert!(seen.insert(tile), "duplicate tile {tile}");
        let (slope, height) = tile_slope_z(&world, tile)?;
        shapes.insert(slope.raw());
        assert_eq!(
            serde_json::json!([slope.raw(), height]),
            row["result"],
            "tile {tile}"
        );
    }
    let expected_shapes = (0_u8..15).chain([23, 27, 29, 30]).collect();
    assert_eq!(
        shapes, expected_shapes,
        "fixture must exercise every native map slope"
    );
    Ok(())
}
