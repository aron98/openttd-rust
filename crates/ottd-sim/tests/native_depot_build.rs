//! Typed command inputs; expected behavior is obtained only from original commands.
use ottd_core::terrain::{Corner, Slope};
use ottd_save::{
    Compression, Savegame, TileRawParts, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{Command, CommandMode, CommandRequest, ReplayAction, ReplayPlan};
use serde_json::json;
use std::path::{Path, PathBuf};
#[path = "depot_cases/mod.rs"]
pub mod cases;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn field(chunk: [u8; 4], record: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}

#[test]
#[ignore = "DEPOT_SOURCE and DEPOT_FLEET_PREP fresh directory"]
fn prepare_fleet_source() -> Result {
    let output = PathBuf::from(std::env::var("DEPOT_FLEET_PREP")?);
    std::fs::create_dir(&output)?;
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(std::env::var("DEPOT_SOURCE")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let tile = u32::try_from(
        world
            .map()
            .tiles()
            .iter()
            .position(|t| {
                t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2 && t.m4().trailing_zeros() >= 6
            })
            .ok_or("road depot")?,
    )?;
    world.edit_batch(vec![
        field(*b"ENGN", 116, "company_avail", WireValue::Unsigned(1)),
        field(
            *b"PATS",
            0,
            "construction.command_pause_level",
            WireValue::Unsigned(3),
        ),
        field(*b"PLYR", 0, "money", WireValue::Signed(1_000_000)),
    ])?;
    save_case(
        &output,
        "fleet",
        &world,
        vec![CommandRequest {
            company: 0,
            mode: CommandMode::Post,
            command: Command::BuildVehicle {
                tile,
                engine: 116,
                cargo: 255,
                use_free_vehicles: false,
                client_id: 0,
            },
        }],
    )
}
const fn request(tile: u32, direction: u8, mode: CommandMode) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode,
        command: Command::BuildRoadDepot {
            tile,
            road_type: 0,
            direction,
        },
    }
}
fn save_case(output: &Path, name: &str, world: &World, requests: Vec<CommandRequest>) -> Result {
    std::fs::write(
        output.join(format!("{name}.sav")),
        world.to_savegame()?.encode(Compression::None)?,
    )?;
    let mut actions = Vec::new();
    for request in requests {
        actions.push(ReplayAction::Command {
            ordinal: u64::try_from(actions.len())?,
            request,
        });
        actions.push(ReplayAction::Checkpoint {
            ordinal: u64::try_from(actions.len())?,
            label: format!("step-{}", actions.len()),
        });
    }
    let plan = ReplayPlan {
        schema_version: 1,
        actions,
    };
    std::fs::write(
        output.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&plan)?,
    )?;
    Ok(())
}
fn clear_pad(world: &World) -> Result<u32> {
    let width = world.map().width();
    let height = world.map().height();
    for y in 4..height.saturating_sub(4) {
        for x in 4..width.saturating_sub(4) {
            let mut clear = true;
            let center = world
                .map()
                .tiles()
                .get(usize::try_from(
                    y.checked_mul(width)
                        .and_then(|v| v.checked_add(x))
                        .ok_or("tile")?,
                )?)
                .ok_or("tile")?
                .height();
            for py in y.saturating_sub(3)..=y.saturating_add(3) {
                for px in x.saturating_sub(3)..=x.saturating_add(3) {
                    let i = py
                        .checked_mul(width)
                        .and_then(|v| v.checked_add(px))
                        .ok_or("tile")?;
                    let tile = world.map().tiles().get(usize::try_from(i)?).ok_or("tile")?;
                    clear &= tile.tile_type() >> 4 == 0
                        && tile.tile_type() & 0x0C == 0
                        && tile.height() == center;
                }
            }
            if clear {
                return Ok(y
                    .checked_mul(width)
                    .and_then(|v| v.checked_add(x))
                    .ok_or("tile")?);
            }
        }
    }
    Err("no clear pad".into())
}
fn slope_edits(world: &World, tile: u32, slope: Slope) -> Result<Vec<WorldEdit>> {
    let width = std::num::NonZeroU32::new(world.map().width()).ok_or("width")?;
    let (x, y) = (tile % width, tile / width);
    let base = world
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?
        .height();
    let points = [
        (x, y, Corner::North),
        (x.saturating_add(1), y, Corner::West),
        (x, y.saturating_add(1), Corner::East),
        (x.saturating_add(1), y.saturating_add(1), Corner::South),
    ];
    let mut edits = Vec::new();
    for py in y.saturating_sub(3)..=y.saturating_add(3) {
        for px in x.saturating_sub(3)..=x.saturating_add(3) {
            let index = py
                .checked_mul(width.get())
                .and_then(|v| v.checked_add(px))
                .ok_or("tile")?;
            let mut raw = TileRawParts::from(
                world
                    .map()
                    .tiles()
                    .get(usize::try_from(index)?)
                    .ok_or("tile")?,
            );
            let mut height = base;
            for (cx, cy, corner) in points {
                let peak = base.checked_add(slope.corner_z(corner)?).ok_or("height")?;
                let distance = px
                    .abs_diff(cx)
                    .checked_add(py.abs_diff(cy))
                    .ok_or("distance")?;
                height = height.max(peak.saturating_sub(u8::try_from(distance)?));
            }
            raw.height = height;
            edits.push(WorldEdit::Tile {
                index,
                value: raw.into(),
            });
        }
    }
    Ok(edits)
}
#[test]
#[ignore = "DEPOT_SOURCE original prepared world and DEPOT_INPUTS fresh output"]
fn prepare_depot_inputs() -> Result {
    let output = PathBuf::from(std::env::var("DEPOT_INPUTS")?);
    std::fs::create_dir(&output)?;
    let mut base = World::decode(&Savegame::decode(
        &std::fs::read(std::env::var("DEPOT_SOURCE")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    base.edit_batch(vec![
        field(
            *b"PATS",
            0,
            "construction.command_pause_level",
            WireValue::Unsigned(3),
        ),
        field(*b"PLYR", 0, "money", WireValue::Signed(1_000_000)),
    ])?;
    let tile = clear_pad(&base)?;
    let mut names = Vec::new();
    for raw in 0..32 {
        let Ok(slope) = Slope::new(raw) else {
            continue;
        };
        if raw == 15 {
            continue;
        }
        for enabled in [false, true] {
            for direction in 0..4 {
                let name = format!("slope-{raw}-enabled-{}-dir-{direction}", u8::from(enabled));
                let mut world = base.clone();
                let mut edits = slope_edits(&world, tile, slope)?;
                edits.push(field(
                    *b"PATS",
                    0,
                    "construction.build_on_slopes",
                    WireValue::Signed(i64::from(enabled)),
                ));
                world.edit_batch(edits)?;
                save_case(
                    &output,
                    &name,
                    &world,
                    vec![
                        request(tile, direction, CommandMode::Estimate),
                        request(tile, direction, CommandMode::Post),
                        request(tile, direction, CommandMode::Post),
                        request(tile, (direction + 1) % 4, CommandMode::Post),
                    ],
                )?;
                names.push(name);
            }
        }
    }
    for ground in 0..6 {
        for snow in [false, true] {
            let name = format!("ground-{ground}-snow-{}", u8::from(snow));
            let mut world = base.clone();
            world.edit_batch(slope_edits(&world, tile, Slope::new(0)?)?)?;
            let mut raw = TileRawParts::from(
                world
                    .map()
                    .tiles()
                    .get(usize::try_from(tile)?)
                    .ok_or("tile")?,
            );
            raw.m5 = (ground << 2) | 3;
            if snow {
                raw.m3 |= 16;
            }
            world.edit_batch(vec![WorldEdit::Tile {
                index: tile,
                value: raw.into(),
            }])?;
            save_case(
                &output,
                &name,
                &world,
                vec![
                    request(tile, 0, CommandMode::Estimate),
                    request(tile, 0, CommandMode::Post),
                ],
            )?;
            names.push(name);
        }
    }
    cases::prepare(&output, &base, tile, &mut names)?;
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({"tile":tile,"cases":names}))?,
    )?;
    Ok(())
}
