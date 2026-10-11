//! Bounded original tree events verified through complete saved-world state-loop calls.
use ottd_save::{Savegame, WireValue, world::World};
use ottd_sim::advance_world;
use serde::Deserialize;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Deserialize)]
struct Event {
    sequence: u64,
    phase: String,
    edge: String,
    tick: u64,
    random: [u32; 2],
    trees_tick_ctr: u64,
    cursor: u64,
    tile: Option<usize>,
    raw: Option<[u64; 10]>,
}
fn number(world: &World, name: &str) -> Result<u64> {
    let table = world.tables().get(b"DATE").ok_or("DATE")?;
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
        _ => Err("DATE field type".into()),
    }
}
fn raw(world: &World, index: usize) -> Result<[u64; 10]> {
    let tile = world.map().tiles().get(index).ok_or("world tile")?;
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
#[test]
#[ignore = "requires frozen native tree replay; run scripts/check-tree-growth.py"]
fn native_tree_events_match_complete_world_ticks() -> Result {
    let directory = PathBuf::from(std::env::var("OTTD_TREE_NATIVE_DIR")?);
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let initial_counter = number(&world, "trees_tick_counter")?;
    let mut last_tick = number(&world, "tick_counter")?;
    let mut pending = Vec::new();
    let mut sequence = 0_u64;
    let mut calls = 0_u64;
    let mut callbacks = 0_u64;
    let mut transitions = std::collections::BTreeSet::new();
    let mut entry = None;
    let mut phase = 0;
    for line in BufReader::new(File::open(directory.join("events.jsonl"))?).lines() {
        let event: Event = serde_json::from_str(&line?)?;
        sequence += 1;
        assert_eq!(event.sequence, sequence);
        if event.phase == "tile_tree" {
            assert_eq!(phase, 0, "tree callbacks must precede vehicle ticks");
            assert_eq!(event.tick, last_tick + 1);
            match event.edge.as_str() {
                "enter" => {
                    assert!(entry.is_none());
                    assert_eq!(
                        event.raw,
                        Some(raw(&world, event.tile.ok_or("tree tile")?)?)
                    );
                    entry = event.raw;
                }
                "leave" => {
                    if let (Some(before), Some(after)) = (entry.take(), event.raw) {
                        transitions.insert((before[6] & 7, after[6] & 7));
                    } else {
                        return Err("unpaired tree event".into());
                    }
                    pending.push(event);
                    callbacks += 1;
                }
                _ => return Err("tree event edge".into()),
            }
            continue;
        }
        if event.phase == "vehicle_ticks" && event.edge == "enter" {
            assert_eq!(phase, 0);
            phase = 1;
            assert_eq!(event.tick, last_tick + 1);
            advance_world(&mut world, 1)?;
            last_tick = event.tick;
            calls += 1;
            for tree in &pending {
                assert_eq!(
                    Some(raw(&world, tree.tile.ok_or("event tile")?)?),
                    tree.raw,
                    "tree callback at tick {}",
                    event.tick
                );
            }
            pending.clear();
            assert_runtime(&world, &event)?;
        }
        match (event.phase.as_str(), event.edge.as_str()) {
            ("vehicle_ticks", "enter") => {}
            ("vehicle_ticks", "leave") => {
                assert_eq!(phase, 1);
                phase = 2;
            }
            ("on_tick_trees", "enter") => {
                assert_eq!(phase, 2);
                phase = 3;
            }
            ("on_tick_trees", "leave") => {
                assert_eq!(phase, 3);
                phase = 0;
            }
            _ => return Err("unexpected phase event".into()),
        }
        assert_eq!(event.tick, last_tick);
        if event.phase == "on_tick_trees" {
            assert_eq!(event.trees_tick_ctr, initial_counter);
            assert_eq!(number(&world, "trees_tick_counter")?, event.trees_tick_ctr);
        }
    }
    assert!(calls > 0 && callbacks > 0);
    assert_eq!(phase, 0);
    assert!(pending.is_empty() && entry.is_none());
    verify_final_checkpoint(
        &world,
        &directory,
        &serde_json::json!({"calls": calls, "callbacks": callbacks, "events": sequence, "transitions": transitions, "whole_world_equal": true, "derived_equal": true, "rng_and_cursor_each_tick_equal": true, "trees_counter_unchanged": initial_counter}),
    )?;
    println!(
        "Compared {calls} complete ticks, {callbacks} tree callbacks and {sequence} bounded native events"
    );
    Ok(())
}

fn verify_final_checkpoint(world: &World, directory: &Path, proof: &serde_json::Value) -> Result {
    let expected: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.world.json"))?)?;
    assert_eq!(world.saved_json()?, expected);
    let expected_derived: serde_json::Value =
        serde_json::from_reader(File::open(directory.join("final.derived.json"))?)?;
    assert_eq!(serde_json::to_value(world.derived())?, expected_derived);
    std::fs::write(
        directory.join("rust-event-proof.json"),
        serde_json::to_vec_pretty(proof)?,
    )?;
    Ok(())
}

fn assert_runtime(world: &World, event: &Event) -> Result {
    assert_eq!(
        number(world, "random_state[0]")?,
        u64::from(event.random[0])
    );
    assert_eq!(
        number(world, "random_state[1]")?,
        u64::from(event.random[1])
    );
    assert_eq!(number(world, "cur_tileloop_tile")?, event.cursor);
    Ok(())
}
