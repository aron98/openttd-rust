#![expect(
    clippy::redundant_pub_crate,
    reason = "helpers belong to one external proof target"
)]
use ottd_save::{TableRecord, TableSchema, WireValue};
use ottd_sim::runtime::SimulationRuntime;
use serde::{Deserialize, Serialize};
use std::{fs::File, path::Path};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Industry {
    id: u16,
    r#type: u8,
    counter: u16,
    prod_level: u8,
    produced: Vec<Produced>,
    accepted: Vec<serde_json::Value>,
    location: [u32; 3],
    town: Option<u32>,
    neutral_station: Option<u32>,
    stations_near: Vec<u16>,
    part_of_subsidy: u8,
    exclusive_consumer: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Produced {
    cargo: u8,
    waiting: u16,
    rate: u8,
    production: u16,
    transported: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Event {
    pub sequence: u64,
    pub phase: String,
    pub edge: String,
    pub tick: u64,
    pub cursor: u32,
    pub random: [u32; 2],
    pub interactive_random: [u32; 2],
    pub daily_counter: u32,
    pub industry: Option<Industry>,
    pub tile: Option<u32>,
    pub raw: Option<[u64; 10]>,
    pub builder: Option<serde_json::Value>,
}

fn value<'a>(schema: &TableSchema, row: &'a TableRecord, name: &str) -> Result<&'a WireValue> {
    schema
        .fields()
        .iter()
        .zip(row.values())
        .find_map(|(f, v)| (f.name() == name).then_some(v))
        .ok_or_else(|| format!("missing {name}").into())
}
fn number(schema: &TableSchema, row: &TableRecord, name: &str) -> Result<u64> {
    match value(schema, row, name)? {
        WireValue::Unsigned(n) => Ok(*n),
        _ => Err(format!("unsigned {name}").into()),
    }
}
fn child<'a>(schema: &'a TableSchema, name: &str) -> Result<&'a TableSchema> {
    schema
        .fields()
        .iter()
        .find(|f| f.name() == name)
        .and_then(ottd_save::FieldSchema::child)
        .ok_or_else(|| format!("child {name}").into())
}
fn records<'a>(
    schema: &TableSchema,
    row: &'a TableRecord,
    name: &str,
) -> Result<&'a [TableRecord]> {
    match value(schema, row, name)? {
        WireValue::Structs(rows) => Ok(rows),
        _ => Err(format!("rows {name}").into()),
    }
}
fn industry(runtime: &SimulationRuntime, id: u32) -> Result<Industry> {
    let table = runtime.world().tables().get(b"INDY").ok_or("INDY")?;
    let row = table.records().get(&id).ok_or("industry")?;
    let schema = table.schema();
    let produced_schema = child(schema, "produced")?;
    let history_schema = child(produced_schema, "history")?;
    let produced = records(schema, row, "produced")?
        .iter()
        .map(|p| {
            let history = records(produced_schema, p, "history")?
                .first()
                .ok_or("history")?;
            Ok(Produced {
                cargo: u8::try_from(number(produced_schema, p, "cargo")?)?,
                waiting: u16::try_from(number(produced_schema, p, "waiting")?)?,
                rate: u8::try_from(number(produced_schema, p, "rate")?)?,
                production: u16::try_from(number(history_schema, history, "production")?)?,
                transported: u16::try_from(number(history_schema, history, "transported")?)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    assert!(records(schema, row, "accepted")?.is_empty());
    for tag in [b"STNN", b"SUBS"] {
        assert!(
            runtime
                .world()
                .tables()
                .get(tag)
                .ok_or("recipient/subsidy pool")?
                .records()
                .is_empty()
        );
    }
    Ok(Industry {
        id: u16::try_from(id)?,
        r#type: u8::try_from(number(schema, row, "type")?)?,
        counter: u16::try_from(number(schema, row, "counter")?)?,
        prod_level: u8::try_from(number(schema, row, "prod_level")?)?,
        produced,
        accepted: Vec::new(),
        location: [
            u32::try_from(number(schema, row, "location.tile")?)?,
            u32::try_from(number(schema, row, "location.w")?)?,
            u32::try_from(number(schema, row, "location.h")?)?,
        ],
        town: u32::try_from(number(schema, row, "town")?)?.checked_sub(1),
        neutral_station: u32::try_from(number(schema, row, "neutral_station")?)?.checked_sub(1),
        stations_near: Vec::new(),
        part_of_subsidy: 0,
        exclusive_consumer: u8::try_from(number(schema, row, "exclusive_consumer")?)?,
    })
}

pub(super) fn checkpoint(runtime: &SimulationRuntime, native: &Path, label: &str) -> Result {
    let observed: serde_json::Value =
        serde_json::from_reader(File::open(native.join(format!("{label}.industries.json")))?)?;
    let world = runtime.saved_json()?;
    let rows = world
        .pointer("/chunks/ITBL/records")
        .ok_or("ITBL")?
        .as_object()
        .ok_or("ITBL rows")?;
    assert_eq!(rows.len(), 240);
    let types = (0..240)
        .map(|i| rows.get(&i.to_string()).ok_or("ITBL type"))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let industries = runtime
        .world()
        .tables()
        .get(b"INDY")
        .ok_or("INDY")?
        .records()
        .keys()
        .map(|id| industry(runtime, *id))
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(
        observed,
        serde_json::json!({
            "industries":industries,"type_members":runtime.industry_caches().type_members,
            "daily_increment":runtime.industry_caches().daily_increment,
            "builder":{"types":types,"wanted_inds":world.pointer("/chunks/IBLD/records/0/wanted_inds").ok_or("wanted industries")?},
        })
    );
    Ok(())
}

pub(super) fn call(runtime: &SimulationRuntime, events: &[Event], tick: u64) -> Result<u64> {
    let mut stack: Vec<&Event> = Vec::new();
    let mut last = None;
    let mut draws = 0_u64;
    for event in events {
        let expected_tick = if matches!(event.phase.as_str(), "daily" | "builder") {
            tick.checked_sub(1).ok_or("economy tick")?
        } else {
            tick
        };
        assert_eq!(event.tick, expected_tick);
        match event.edge.as_str() {
            "enter" => stack.push(event),
            "leave" => {
                let before = stack.pop().ok_or("industry event stack")?;
                assert_eq!((&before.phase, before.tile), (&event.phase, event.tile));
                assert_eq!(before.cursor, event.cursor);
                assert_eq!(before.interactive_random, event.interactive_random);
                match event.phase.as_str() {
                    "tile_loop" | "production" => {
                        assert_eq!(before.sequence.checked_add(1), Some(event.sequence));
                        let mut rng = ottd_core::Randomizer::from_state(before.random);
                        let mut count = 0_u64;
                        while rng.state() != event.random && count < 4 {
                            let _draw = rng.next_u32();
                            count = count.checked_add(1).ok_or("draw count")?;
                        }
                        assert_eq!(rng.state(), event.random, "{} draws", event.phase);
                        draws = draws.checked_add(count).ok_or("draw total")?;
                        if let Some(tile) = event.tile {
                            assert_eq!(
                                Some(super::house_events::raw(runtime.world(), tile)?),
                                event.raw
                            );
                        }
                        last = event.industry.as_ref();
                    }
                    "daily" | "builder" => {
                        assert!(event.builder.is_some());
                        assert_eq!(
                            u64::from(event.daily_counter),
                            super::house_events::number(
                                runtime.world(),
                                *b"ECMY",
                                0,
                                "industry_daily_change_counter"
                            )?
                        );
                    }
                    _ => return Err("unexpected native industry phase".into()),
                }
            }
            _ => return Err("unexpected native industry edge".into()),
        }
    }
    assert!(stack.is_empty());
    if let Some(last) = last {
        assert_eq!(industry(runtime, u32::from(last.id))?, *last);
    }
    Ok(draws)
}
