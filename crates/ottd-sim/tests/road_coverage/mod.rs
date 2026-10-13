//! Source-backed road restoration admission boundaries and sparse-ID witnesses.
use ottd_save::{
    TileRawParts, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::runtime::{RuntimeError, SimulationRuntime};
use std::{collections::BTreeSet, fmt::Write, path::Path};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Assert every admitted tile branch and a high sparse native vehicle ID.
/// # Errors
/// Returns missing world data or artifact write errors.
/// # Panics
/// Fails the test when sparse IDs or admitted tile branches are absent.
pub fn assert_admitted(runtime: &SimulationRuntime, directory: &Path) -> Result {
    let ids: Vec<_> = runtime.road_caches().keys().map(|id| id.raw()).collect();
    assert!(
        ids.last().is_some_and(|id| *id >= 128),
        "high native vehicle ID missing"
    );
    assert!(
        ids.windows(2).any(|pair| pair
            .first()
            .zip(pair.get(1))
            .is_some_and(|(a, b)| b.saturating_sub(*a) > 1)),
        "sparse native vehicle ID missing"
    );
    let mut branches = BTreeSet::new();
    for id in runtime.road_caches().keys() {
        let tile = runtime.vehicle(*id)?.tile()?;
        let tile = runtime
            .world()
            .map()
            .tiles()
            .get(usize::try_from(tile)?)
            .ok_or("tile")?;
        let branch = match tile.tile_type() >> 4 {
            2 => match tile.m5() >> 6 {
                0 => "road",
                1 => "crossing",
                2 => "depot",
                _ => "unknown-road",
            },
            5 => match (tile.m6() >> 3) & 15 {
                2 => "truck",
                3 => "bus",
                8 => "waypoint",
                _ => "unknown-station",
            },
            9 => {
                if tile.m5() & 128 == 0 {
                    "tunnel"
                } else {
                    "bridge"
                }
            }
            _ => "non-road",
        };
        branches.insert(branch);
    }
    assert_eq!(
        branches,
        BTreeSet::from([
            "road", "crossing", "depot", "bus", "truck", "waypoint", "bridge", "tunnel"
        ])
    );
    std::fs::write(
        directory.join("admission.json"),
        serde_json::to_vec(&serde_json::json!({"ids":ids,"branches":branches}))?,
    )?;
    Ok(())
}

fn common(id: u32, name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value: WireValue::Unsigned(value),
    }
}

/// Reject precise unsupported tile and vehicle shapes after valid saved edits.
/// # Errors
/// Returns invalid fixture edits or missing fixture fields.
/// # Panics
/// Fails the test if an unsupported shape reports the wrong runtime error.
pub fn assert_rejections(world: &World, directory: &Path) -> Result {
    let runtime = SimulationRuntime::restore_vanilla(world.clone())?;
    let first = *runtime.road_caches().keys().next().ok_or("vehicle")?;
    let second = *runtime
        .road_caches()
        .keys()
        .nth(1)
        .ok_or("second vehicle")?;
    let tile_id = runtime.vehicle(first)?.tile()?;
    let tile = runtime
        .world()
        .map()
        .tiles()
        .get(usize::try_from(tile_id)?)
        .ok_or("tile")?;
    let base = TileRawParts::from(tile);
    let mut cases = Vec::new();
    let mut plain = base;
    plain.tile_type = 0;
    cases.push((
        "clear-tile",
        WorldEdit::Tile {
            index: tile_id,
            value: plain.into(),
        },
        "vehicle tile lacks vanilla road",
    ));
    for (name, kind, m5, m6, m4) in [
        ("rail-station", 5, 0, 0, 0),
        ("non-road-bridge", 9, 128, 0, 0),
        ("no-road-type", 2, 0, 0, 63),
        ("custom-road-type", 2, 0, 0, 1),
    ] {
        let mut tile = base;
        tile.tile_type = kind << 4;
        tile.m5 = m5;
        tile.m6 = m6;
        tile.m4 = m4;
        tile.m2 = 0;
        cases.push((
            name,
            WorldEdit::Tile {
                index: tile_id,
                value: tile.into(),
            },
            "vehicle tile lacks vanilla road",
        ));
    }
    cases.push((
        "non-front",
        common(first.raw(), "subtype", 0),
        "non-front or articulated road vehicle",
    ));
    cases.push((
        "articulated-bit",
        common(first.raw(), "subtype", 2),
        "non-front or articulated road vehicle",
    ));
    cases.push((
        "linked-chain",
        common(
            first.raw(),
            "next",
            u64::from(second.raw()).saturating_add(1),
        ),
        "non-front or articulated road vehicle",
    ));
    let mut evidence = String::new();
    for (name, edit, expected) in cases {
        let mut changed = world.clone();
        changed.edit_batch(vec![edit])?;
        let error = SimulationRuntime::restore_vanilla(changed)
            .err()
            .ok_or("unsupported shape admitted")?;
        assert!(
            matches!(error,RuntimeError::Unsupported(reason) if reason==expected),
            "wrong rejection for {name}: {error}"
        );
        writeln!(evidence, "PASS {name}: {error}")?;
    }
    std::fs::write(directory.join("rejections.txt"), evidence)?;
    Ok(())
}
