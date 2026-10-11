#![expect(
    clippy::redundant_pub_crate,
    reason = "private event helpers are visible only to the parent proof target"
)]
use ottd_save::{WireValue, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoopEvent {
    pub sequence: u64,
    pub call: u32,
    pub edge: String,
    pub tick: u64,
    pub cursor: u32,
    pub random: [u32; 2],
    pub interactive_random: [u32; 2],
    pub script_none_random: [u32; 2],
    pub trees_tick_counter: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HouseEvent {
    pub sequence: u64,
    pub call: u32,
    pub edge: String,
    pub tile: u32,
    pub tick: u64,
    pub cursor: u32,
    pub random: [u32; 2],
    pub interactive_random: [u32; 2],
    pub raw: [u64; 10],
    pub footprint: Vec<ObservedTile>,
    pub town: u16,
    pub population: u32,
    pub supplied: Vec<Supplied>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ObservedTile {
    pub tile: u32,
    pub raw: [u64; 10],
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Supplied {
    pub cargo: u8,
    pub production: u32,
    pub transported: u32,
}

pub(super) fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    BufReader::new(File::open(path)?)
        .lines()
        .map(|line| Ok(serde_json::from_str(&line?)?))
        .collect()
}

pub(super) fn number(world: &World, chunk: [u8; 4], id: u32, name: &str) -> Result<u64> {
    let table = world.tables().get(&chunk).ok_or("table")?;
    let row = table.records().get(&id).ok_or("record")?;
    match table
        .schema()
        .fields()
        .iter()
        .zip(row.values())
        .find_map(|(field, value)| (field.name() == name).then_some(value))
        .ok_or("field")?
    {
        WireValue::Unsigned(value) => Ok(*value),
        _ => Err("unsigned native field".into()),
    }
}

pub(super) fn loop_equal(runtime: &SimulationRuntime, event: &LoopEvent) -> Result {
    let world = runtime.world();
    assert_eq!(number(world, *b"DATE", 0, "tick_counter")?, event.tick);
    assert_eq!(
        number(world, *b"DATE", 0, "cur_tileloop_tile")?,
        u64::from(event.cursor)
    );
    for (name, value) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(event.random)
    {
        assert_eq!(
            number(world, *b"DATE", 0, name)?,
            u64::from(value),
            "call {}",
            event.call
        );
    }
    for (name, value) in ["state[0]", "state[1]"]
        .into_iter()
        .zip(event.script_none_random)
    {
        assert_eq!(number(world, *b"SRND", 16, name)?, u64::from(value));
    }
    assert_eq!(
        number(world, *b"DATE", 0, "trees_tick_counter")?,
        u64::from(event.trees_tick_counter)
    );
    Ok(())
}

pub(super) fn raw(world: &World, index: u32) -> Result<[u64; 10]> {
    let tile = world
        .map()
        .tiles()
        .get(usize::try_from(index)?)
        .ok_or("house tile")?;
    Ok([
        u64::from(tile.tile_type()),
        u64::from(tile.height()),
        u64::from(tile.m1()),
        u64::from(tile.m2()),
        u64::from(tile.m3()),
        u64::from(tile.m4()),
        u64::from(tile.m5()),
        u64::from(tile.m6()),
        u64::from(tile.m7()),
        u64::from(tile.m8()),
    ])
}

pub(super) fn scheduled_houses(runtime: &SimulationRuntime, next_tick: u64) -> Result<Vec<u32>> {
    let world = runtime.world();
    // Pinned landscape.cpp RunTileLoop: sum of map exponents selects feedback.
    let (feedback, count) = match (world.map().width(), world.map().height()) {
        (64, 64) => (0x0d8f, 16),
        (128, 128) => (0x2496, 64),
        _ => return Err("map shape outside this native proof corpus".into()),
    };
    if number(world, *b"DATE", 0, "pause_mode")? != 0 {
        return Ok(Vec::new());
    }
    let mut cursor = u32::try_from(number(world, *b"DATE", 0, "cur_tileloop_tile")?)?;
    let mut visits = Vec::new();
    if next_tick % 256 == 0 {
        visits.push(0);
    }
    for _ in visits.len()..count {
        visits.push(cursor);
        cursor = (cursor >> 1) ^ if cursor & 1 == 1 { feedback } else { 0 };
    }
    visits
        .into_iter()
        .filter_map(|index| {
            let tile = usize::try_from(index)
                .ok()
                .and_then(|i| world.map().tiles().get(i));
            match tile {
                Some(tile) if tile.tile_type() >> 4 == 3 => Some(Ok(index)),
                Some(_) => None,
                None => Some(Err("scheduled tile".into())),
            }
        })
        .collect()
}

pub(super) fn houses_equal(
    runtime: &SimulationRuntime,
    events: &[HouseEvent],
    expected_visits: &[u32],
    call: u32,
    tick: u64,
) -> Result<u64> {
    assert_eq!(events.len() % 2, 0);
    assert_eq!(events.len() / 2, expected_visits.len());
    let mut tiles = BTreeMap::new();
    let mut towns = BTreeMap::new();
    let mut draws = 0_u64;
    for (pair, expected_tile) in events.chunks_exact(2).zip(expected_visits) {
        let before = pair.first().ok_or("house before")?;
        let after = pair.get(1).ok_or("house after")?;
        assert_eq!(before.edge, "enter");
        assert_eq!(after.edge, "leave");
        assert_eq!(before.sequence.checked_add(1), Some(after.sequence));
        assert_eq!((before.call, after.call), (call, call));
        assert_eq!((before.tick, after.tick), (tick, tick));
        assert_eq!(before.cursor, after.cursor);
        assert_eq!(before.tile, after.tile);
        assert_eq!(before.tile, *expected_tile);
        assert_eq!(before.interactive_random, after.interactive_random);
        assert_eq!(
            before.footprint.first().ok_or("before footprint")?.raw,
            before.raw
        );
        assert_eq!(
            after.footprint.first().ok_or("after footprint")?.raw,
            after.raw
        );
        let mut random = ottd_core::Randomizer::from_state(before.random);
        let mut count = 0_u64;
        while random.state() != after.random && count <= 5 {
            let _ = random.next_u32();
            count = count.checked_add(1).ok_or("draw count")?;
        }
        assert!(count <= 5);
        assert_eq!(
            count == 0,
            before.raw.get(4).ok_or("completion bit")? & 128 == 0
        );
        draws = draws.checked_add(count).ok_or("draw total")?;
        for tile in &after.footprint {
            tiles.insert(tile.tile, tile.raw);
        }
        towns.insert(after.town, after);
    }
    for (index, expected) in tiles {
        assert_eq!(
            raw(runtime.world(), index)?,
            expected,
            "call {call}, tile {index}"
        );
    }
    for (id, event) in towns {
        let cache = runtime
            .town_house_caches()
            .towns
            .iter()
            .find(|town| town.id == id)
            .ok_or("town cache")?;
        assert_eq!(cache.population, event.population, "call {call}, town {id}");
        let saved = runtime.saved_json()?;
        let rows = saved
            .pointer(&format!("/chunks/CITY/records/{id}/supplied"))
            .and_then(serde_json::Value::as_array)
            .ok_or("supplied rows")?;
        let actual = rows
            .iter()
            .map(|row| {
                Ok(Supplied {
                    cargo: u8::try_from(
                        row.get("cargo")
                            .and_then(serde_json::Value::as_u64)
                            .ok_or("cargo")?,
                    )?,
                    production: u32::try_from(
                        row.pointer("/history/0/production")
                            .and_then(serde_json::Value::as_u64)
                            .ok_or("production")?,
                    )?,
                    transported: u32::try_from(
                        row.pointer("/history/0/transported")
                            .and_then(serde_json::Value::as_u64)
                            .ok_or("transported")?,
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(actual, event.supplied, "call {call}, town {id}");
    }
    Ok(draws)
}
