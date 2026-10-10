use super::{Result, checkpoint, write};
use ottd_save::{
    Compression, Savegame, TableRecord, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{WorldTickError, runtime::SimulationRuntime};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mutation {
    OtherSide,
    Direction,
    Stopped,
    UncoveredEngine,
    Turn,
    OffsetMonth,
    DailyBreakdown,
    ZeroSpeed,
    ActiveOrder,
    ActiveBreakdown,
    SavedPath,
    RealisticAcceleration,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    input: PathBuf,
    output: PathBuf,
    subject: u32,
    mutation: Mutation,
    crossing: u32,
}

fn common(subject: u32, field: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: subject,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(field.into()),
        ],
        value: WireValue::Unsigned(value),
    }
}

fn turn(world: &World, subject: u32) -> Result<WorldEdit> {
    let saved = world.saved_json()?;
    let tile = saved
        .pointer(&format!(
            "/chunks/VEHS/records/{subject}/roadveh/0/common/0/tile"
        ))
        .and_then(serde_json::Value::as_u64)
        .ok_or("vehicle tile missing")?;
    let index = u32::try_from(tile.checked_sub(1).ok_or("tile underflow")?)?;
    let mut raw: ottd_save::TileRawParts = world
        .map()
        .tiles()
        .get(usize::try_from(index)?)
        .ok_or("tile bounds")?
        .into();
    raw.m5 = 2;
    Ok(WorldEdit::Tile {
        index,
        value: raw.into(),
    })
}

fn mutate(world: &mut World, request: &Request) -> Result<(u32, Option<u32>)> {
    let mut edits = Vec::new();
    let (horizon, prefix) = match request.mutation {
        Mutation::ZeroSpeed => {
            edits.push(common(request.subject, "cur_speed", 0));
            (1, None)
        }
        Mutation::ActiveOrder => {
            edits.push(common(request.subject, "current_order.type", 1));
            (1, None)
        }
        Mutation::ActiveBreakdown => {
            edits.push(common(request.subject, "breakdown_ctr", 1));
            (1, None)
        }
        Mutation::SavedPath => {
            edits.push(WorldEdit::StructList {
                chunk: *b"VEHS",
                record: request.subject,
                path: vec![
                    PathElement::Field("roadveh".into()),
                    PathElement::Index(0),
                    PathElement::Field("path".into()),
                ],
                rows: vec![TableRecord::new(vec![
                    WireValue::Unsigned(0),
                    WireValue::Unsigned(670),
                ])],
            });
            (1, None)
        }
        Mutation::RealisticAcceleration => {
            edits.push(WorldEdit::Field {
                chunk: *b"PATS",
                record: 0,
                path: vec![PathElement::Field(
                    "vehicle.roadveh_acceleration_model".into(),
                )],
                value: WireValue::Unsigned(1),
            });
            (1, None)
        }
        Mutation::OtherSide => {
            edits.push(WorldEdit::Field {
                chunk: *b"PATS",
                record: 0,
                path: vec![PathElement::Field("vehicle.road_side".into())],
                value: WireValue::Unsigned(0),
            });
            (1, None)
        }
        Mutation::Direction => {
            edits.push(common(request.subject, "direction", 3));
            (1, None)
        }
        Mutation::Stopped => {
            edits.push(common(request.subject, "vehstatus", 2));
            (1, None)
        }
        Mutation::UncoveredEngine => {
            edits.push(common(request.subject, "engine_type", 117));
            (1, None)
        }
        Mutation::DailyBreakdown => {
            edits.push(common(request.subject, "breakdown_chance", 255));
            (74, None)
        }
        Mutation::Turn => {
            edits.push(turn(world, request.subject)?);
            (request.crossing, None)
        }
        Mutation::OffsetMonth => {
            let day = if request.crossing < 74 { 31 } else { 30 };
            let date = ottd_core::CalendarDate::from_ymd(2100, 0, day)?.raw();
            edits.push(WorldEdit::Field {
                chunk: *b"DATE",
                record: 0,
                path: vec![PathElement::Field("economy_date".into())],
                value: WireValue::Signed(i64::from(date)),
            });
            edits.push(WorldEdit::Field {
                chunk: *b"DATE",
                record: 0,
                path: vec![PathElement::Field("economy_date_fract".into())],
                value: WireValue::Unsigned(0),
            });
            (if day == 31 { 74 } else { 148 }, Some(request.crossing))
        }
    };
    world.edit_batch(edits)?;
    Ok((horizon, prefix))
}

#[test]
#[ignore = "requires fresh original-generated movement corpus and mandatory driver invocation"]
fn whole_horizon_refusal() -> Result {
    let request: Request = serde_json::from_slice(&std::fs::read(std::env::var(
        "OTTD_MOVEMENT_RUST_CONTROL",
    )?)?)?;
    if request.crossing == 0
        || request.crossing > 128
        || !request.input.is_absolute()
        || !request.output.is_absolute()
    {
        return Err("invalid movement refusal request".into());
    }
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(&request.input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let (horizon, prefix_calls) = mutate(&mut world, &request)?;
    let mut prefix = SimulationRuntime::restore_vanilla(world.clone())?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    std::fs::create_dir(&request.output)?;
    checkpoint(&runtime, &request.output, "before")?;
    let initial = runtime.saved_json()?;
    let physical = runtime.road_caches().clone();
    let spatial = runtime.single_road_tile_occupancy()?;
    assert!(runtime.advance_world(0)?.ticks.is_empty());
    assert_eq!(runtime.saved_json()?, initial);
    assert_eq!(runtime.road_caches(), &physical);
    assert_eq!(runtime.single_road_tile_occupancy()?, spatial);
    if let Some(calls) = prefix_calls {
        prefix.advance_world(calls)?;
        let advanced = prefix.saved_json()?;
        assert_ne!(
            advanced.pointer("/chunks/DATE/records/0/random_state[0]"),
            initial.pointer("/chunks/DATE/records/0/random_state[0]")
        );
        assert_ne!(
            advanced.pointer("/chunks/VEHS"),
            initial.pointer("/chunks/VEHS")
        );
        checkpoint(&prefix, &request.output, "prefix")?;
    }
    let result = runtime.advance_world(horizon);
    assert!(matches!(&result, Err(WorldTickError::Unsupported { .. })));
    assert_eq!(runtime.saved_json()?, initial);
    assert_eq!(runtime.road_caches(), &physical);
    assert_eq!(runtime.single_road_tile_occupancy()?, spatial);
    checkpoint(&runtime, &request.output, "after")?;
    write(
        &request.output.join("negative.sav"),
        &runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    write(
        &request.output.join("refusal.json"),
        &serde_json::to_vec(&serde_json::json!({
            "horizon": horizon, "prefix_calls": prefix_calls, "error": format!("{result:?}"),
            "saved_unchanged": true, "physical_unchanged": true, "spatial_unchanged": true,
        }))?,
    )?;
    Ok(())
}
