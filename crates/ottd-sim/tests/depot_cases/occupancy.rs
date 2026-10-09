//! Ground vehicle positions from an original-created road fleet.
use super::*;
use ottd_sim::runtime::SimulationRuntime;

/// Write paired north/minimum versus maximum-height occupancy inputs.
/// # Errors
/// Rejects missing native fleet/depot data or invalid saved edits.
pub fn prepare(output: &Path, names: &mut Vec<String>) -> Result {
    let Fleet {
        base,
        vehicles,
        target,
    } = fleet()?;
    for name in [
        "occupied-flat",
        "occupied-max-corner",
        "owner-before-occupancy",
        "slope-before-owner",
    ] {
        let mut world = base.clone();
        let mut depot = TileRawParts::from(
            world
                .map()
                .tiles()
                .get(usize::try_from(target)?)
                .ok_or("tile")?,
        );
        depot.m5 &= !3;
        if matches!(name, "owner-before-occupancy" | "slope-before-owner") {
            depot.m1 = (depot.m1 & !31) | 1;
        }
        let mut edits = vec![
            WorldEdit::Tile {
                index: target,
                value: depot.into(),
            },
            field(
                *b"PATS",
                0,
                "construction.command_pause_level",
                WireValue::Unsigned(3),
            ),
            field(
                *b"PATS",
                0,
                "construction.build_on_slopes",
                WireValue::Signed(i64::from(name != "slope-before-owner")),
            ),
        ];
        let base_height = world
            .map()
            .tiles()
            .get(usize::try_from(target)?)
            .ok_or("tile")?
            .height();
        let offset = i64::from(name != "occupied-flat");
        if offset != 0 {
            let east = target.checked_add(world.map().width()).ok_or("east")?;
            let mut corner = TileRawParts::from(
                world
                    .map()
                    .tiles()
                    .get(usize::try_from(east)?)
                    .ok_or("east")?,
            );
            corner.height = base_height.checked_add(1).ok_or("height")?;
            edits.push(WorldEdit::Tile {
                index: east,
                value: corner.into(),
            });
        }
        let z = i64::from(base_height)
            .saturating_add(offset)
            .saturating_mul(8);
        for (id, tile) in &vehicles {
            if *tile == target {
                edits.push(vehicle_height(*id, z));
            }
        }
        world.edit_batch(edits)?;
        save_case(
            output,
            name,
            &world,
            vec![
                request(target, 0, CommandMode::Post),
                request(target, 1, CommandMode::Estimate),
                request(target, 1, CommandMode::Post),
            ],
        )?;
        names.push(name.into());
    }
    Ok(())
}

struct Fleet {
    base: World,
    vehicles: Vec<(u32, u32)>,
    target: u32,
}
fn fleet() -> Result<Fleet> {
    let source = std::env::var("DEPOT_FLEET")?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(source)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let runtime = SimulationRuntime::restore_vanilla(world)?;
    let mut vehicles = Vec::new();
    for id in runtime.road_caches().keys() {
        vehicles.push((id.raw(), runtime.vehicle(*id)?.tile()?));
    }
    let base = runtime.into_world();
    let target = vehicles
        .iter()
        .map(|(_, tile)| *tile)
        .find(|tile| {
            base.map()
                .tiles()
                .get(usize::try_from(*tile).unwrap_or(usize::MAX))
                .is_some_and(|t| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
        })
        .ok_or("occupied original depot")?;
    Ok(Fleet {
        base,
        vehicles,
        target,
    })
}

fn vehicle_height(id: u32, z: i64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field("z_pos".into()),
        ],
        value: WireValue::Signed(z),
    }
}
