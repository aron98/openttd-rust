//! Compact original mutation events checked through complete saved-world calls.
use ottd_save::{Savegame, WireValue, world::World};
use ottd_sim::advance_world;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[path = "native_world_tree_spread/trace.rs"]
mod trace;
use trace::{Event, Trace};

fn field(world: &World, chunk: [u8; 4], name: &str) -> Result<u64> {
    let table = world.tables().get(&chunk).ok_or("table")?;
    let row = table.records().get(&0).ok_or("DATE row")?;
    match table
        .schema()
        .fields()
        .iter()
        .zip(row.values())
        .find_map(|(f, v)| (f.name() == name).then_some(v))
        .ok_or("DATE field")?
    {
        WireValue::Unsigned(value) => Ok(*value),
        _ => Err("DATE type".into()),
    }
}
fn number(world: &World, name: &str) -> Result<u64> {
    field(world, *b"DATE", name)
}
fn raw(world: &World, index: usize) -> Result<[u64; 10]> {
    let t = world.map().tiles().get(index).ok_or("tile")?;
    Ok([
        u64::from(t.tile_type()),
        u64::from(t.height()),
        u64::from(t.m1()),
        u64::from(t.m2()),
        u64::from(t.m3()),
        u64::from(t.m4()),
        u64::from(t.m5()),
        u64::from(t.m6()),
        u64::from(t.m7()),
        u64::from(t.m8()),
    ])
}
fn full_checkpoint(world: &World, directory: &Path) -> Result {
    let saved: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.world.json"))?)?;
    assert_eq!(world.saved_json()?, saved);
    let derived: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.derived.json"))?)?;
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    Ok(())
}

#[test]
#[ignore = "requires a complete original spreading tree capture"]
fn native_spreading_events_match_complete_world_ticks() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_TREE_SPREAD_NATIVE_DIR")?);
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut trace = Trace::new(number(&world, "tick_counter")?);
    let spread = field(&world, *b"PATS", "construction.extra_tree_placement")? == 2;
    for line in BufReader::new(File::open(directory.join("events.jsonl"))?).lines() {
        let event: Event = serde_json::from_str(&line?)?;
        if let Some(event) = trace.observe(event, spread)? {
            advance_world(&mut world, 1)?;
            assert_eq!(number(&world, "tick_counter")?, event.tick);
            assert_eq!(
                number(&world, "random_state[0]")?,
                u64::from(event.random[0])
            );
            assert_eq!(
                number(&world, "random_state[1]")?,
                u64::from(event.random[1])
            );
            assert_eq!(number(&world, "cur_tileloop_tile")?, event.cursor);
            assert_eq!(number(&world, "trees_tick_counter")?, event.trees_tick_ctr);
            for (index, value) in &trace.expected {
                assert_eq!(
                    raw(&world, *index)?,
                    *value,
                    "native touched tile {index}, tick {}",
                    event.tick
                );
            }
            trace.expected.clear();
        }
    }
    trace.complete();
    full_checkpoint(&world, &directory)?;
    let mut proof = serde_json::to_value(&trace.stats)?;
    let fields = proof.as_object_mut().ok_or("statistics object")?;
    for name in [
        "whole_world_equal",
        "derived_equal",
        "rng_cursor_counter_and_touched_tiles_each_complete_call",
    ] {
        fields.insert(name.into(), true.into());
    }
    std::fs::write(
        directory.join("rust-spread-proof.json"),
        serde_json::to_vec_pretty(&proof)?,
    )?;
    println!(
        "Compared {} complete calls, {} original events, {} plants",
        trace.stats.calls, trace.stats.events, trace.stats.plants
    );
    Ok(())
}
