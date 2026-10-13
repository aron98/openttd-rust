//! Additional typed depot command inputs.
use super::*;
pub mod occupancy;

/// Write ownership, pricing, argument and existing-infrastructure cases.
/// # Errors
/// Rejects missing original fixture structures or invalid saved edits.
pub fn prepare(output: &Path, base: &World, tile: u32, names: &mut Vec<String>) -> Result {
    let (depot_index, depot) = base
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2 && t.m4().trailing_zeros() >= 6)
        .ok_or("original road depot")?;
    let depot_index = u32::try_from(depot_index)?;
    for name in [
        "money",
        "pause",
        "nonowner",
        "no-op-negative-cash",
        "naming-hole",
        "raw-errors",
        "second-company",
    ] {
        let mut world = base.clone();
        world.edit_batch(slope_edits(&world, tile, Slope::new(0)?)?)?;
        let mut edits = Vec::new();
        let target = match name {
            "money" => {
                edits.push(field(*b"PLYR", 0, "money", WireValue::Signed(0)));
                tile
            }
            "pause" => {
                edits.push(field(
                    *b"PATS",
                    0,
                    "construction.command_pause_level",
                    WireValue::Unsigned(0),
                ));
                tile
            }
            "nonowner" => {
                let mut t = TileRawParts::from(depot);
                t.m1 = (t.m1 & !31) | 1;
                edits.push(WorldEdit::Tile {
                    index: depot_index,
                    value: t.into(),
                });
                depot_index
            }
            "no-op-negative-cash" => {
                edits.push(field(*b"PLYR", 0, "money", WireValue::Signed(-1)));
                depot_index
            }
            "naming-hole" => {
                for id in world.tables().get(b"DEPT").ok_or("DEPT")?.records().keys() {
                    edits.push(field(
                        *b"DEPT",
                        *id,
                        "town_cn",
                        WireValue::Unsigned(u64::from(*id) + 2),
                    ));
                }
                tile
            }
            "raw-errors" | "second-company" => tile,
            _ => return Err("case".into()),
        };
        world.edit_batch(edits)?;
        let dir = if target == depot_index {
            depot.m5() & 3
        } else {
            0
        };
        let mut requests = vec![
            request(target, dir, CommandMode::Estimate),
            request(target, dir, CommandMode::Post),
            request(target, dir.wrapping_add(1) % 4, CommandMode::Post),
        ];
        if name == "second-company" {
            for request in &mut requests {
                request.company = 1;
            }
        }
        if name == "raw-errors" {
            for (road_type, direction) in [(1, 0), (63, 0), (255, 0), (0, 4), (0, 255)] {
                requests.push(CommandRequest {
                    company: 0,
                    mode: CommandMode::Post,
                    command: Command::BuildRoadDepot {
                        tile,
                        road_type,
                        direction,
                    },
                });
            }
            let mut invalid_company = request(tile, 0, CommandMode::Post);
            invalid_company.company = 14;
            requests.push(invalid_company);
            requests.push(request(u32::MAX, 0, CommandMode::Post));
        }
        save_case(output, name, &world, requests)?;
        names.push(name.into());
    }
    existing_surfaces(output, base, names)?;
    occupancy::prepare(output, names)
}
fn existing_surfaces(output: &Path, base: &World, names: &mut Vec<String>) -> Result {
    let tram = base
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2 && (t.m8() >> 6) & 63 == 1)
        .ok_or("original tram depot")?
        .0;
    save_case(
        output,
        "different-depot-type",
        base,
        vec![request(u32::try_from(tram)?, 0, CommandMode::Post)],
    )?;
    names.push("different-depot-type".into());
    let tram = u32::try_from(tram)?;
    let mut sloped = base.clone();
    let east = tram.checked_add(base.map().width()).ok_or("east")?;
    let mut corner = TileRawParts::from(
        base.map()
            .tiles()
            .get(usize::try_from(east)?)
            .ok_or("east")?,
    );
    corner.height = corner.height.checked_add(1).ok_or("height")?;
    sloped.edit_batch(vec![
        WorldEdit::Tile {
            index: east,
            value: corner.into(),
        },
        field(
            *b"PATS",
            0,
            "construction.build_on_slopes",
            WireValue::Signed(1),
        ),
    ])?;
    save_case(
        output,
        "different-depot-type-sloped",
        &sloped,
        vec![
            request(tram, 0, CommandMode::Estimate),
            request(tram, 0, CommandMode::Post),
        ],
    )?;
    names.push("different-depot-type-sloped".into());
    let bridge = base
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 0 && t.tile_type() & 0x0C != 0)
        .ok_or("original bridge above clear tile")?
        .0;
    save_case(
        output,
        "bridge-above",
        base,
        vec![request(u32::try_from(bridge)?, 0, CommandMode::Post)],
    )?;
    names.push("bridge-above".into());
    Ok(())
}
