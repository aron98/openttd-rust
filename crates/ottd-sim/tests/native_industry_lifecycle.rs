//! Actual whole-world industry lifecycle, complete checkpoints and compact callback edges.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde::Deserialize;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
#[path = "native_house_lifecycle/events.rs"]
mod house_events;
#[path = "native_industry_lifecycle/observe.rs"]
mod observe;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Protocol {
    schema_version: u8,
    actions: Vec<Action>,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Tick { ordinal: u32, count: u32 },
    Checkpoint { ordinal: u32, label: String },
}

fn write(path: &Path, bytes: &[u8]) -> Result {
    let mut file = File::create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn checkpoint(runtime: &SimulationRuntime, native: &Path, output: &Path, label: &str) -> Result {
    for (view, actual) in [
        ("world", runtime.saved_json()?),
        ("derived", serde_json::to_value(runtime.world().derived())?),
        ("houses", serde_json::to_value(runtime.town_house_caches())?),
    ] {
        let name = format!("{label}.{view}.json");
        write(&output.join(&name), &serde_json::to_vec(&actual)?)?;
        let expected: serde_json::Value = serde_json::from_reader(File::open(native.join(name))?)?;
        assert_eq!(actual, expected, "{label}/{view}");
    }
    observe::checkpoint(runtime, native, label)?;
    write(
        &output.join(format!("{label}.industry-cache.json")),
        &serde_json::to_vec(runtime.industry_caches())?,
    )?;
    write(
        &output.join(format!("{label}.sav")),
        &runtime.to_savegame()?.encode(Compression::Zlib)?,
    )
}

#[test]
#[ignore = "requires original industry, house and whole-loop events"]
fn native_industry_lifecycle_matches_complete_runtime() -> Result {
    let native = PathBuf::from(std::env::var("OTTD_INDUSTRY_NATIVE_DIR")?);
    let output = PathBuf::from(std::env::var("OTTD_INDUSTRY_RUST_OUTPUT")?);
    std::fs::create_dir(&output)?;
    let protocol: Protocol = serde_json::from_reader(File::open(native.join("protocol.json"))?)?;
    assert_eq!(protocol.schema_version, 1);
    let world = World::decode(&Savegame::decode(
        &std::fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    checkpoint(&runtime, &native, &output, "initial")?;
    let loops = house_events::read::<house_events::LoopEvent>(&native.join("world-events.jsonl"))?;
    let houses = house_events::read::<house_events::HouseEvent>(&native.join("events.jsonl"))?;
    let industries = house_events::read::<observe::Event>(&native.join("industry-events.jsonl"))?;
    for (index, event) in industries.iter().enumerate() {
        assert_eq!(
            event.sequence,
            u64::try_from(index)?.checked_add(1).ok_or("sequence")?
        );
    }
    let mut loops = loops.iter();
    let (mut hp, mut ip, mut calls, mut boundaries, mut draws) = (0, 0, 0_u32, 2_u32, 0_u64);
    for (index, action) in protocol.actions.into_iter().enumerate() {
        match action {
            Action::Tick { ordinal, count } => {
                assert_eq!(usize::try_from(ordinal)?, index);
                if count == 0 {
                    assert!(runtime.advance_world(0)?.ticks.is_empty());
                }
                for _ in 0..count {
                    calls += 1;
                    let before = loops.next().ok_or("loop enter")?;
                    let after = loops.next().ok_or("loop leave")?;
                    assert_eq!((&*before.edge, &*after.edge), ("enter", "leave"));
                    assert_eq!((before.call, after.call), (calls, calls));
                    assert_eq!(before.sequence.checked_add(1), Some(after.sequence));
                    assert_eq!(before.interactive_random, after.interactive_random);
                    house_events::loop_equal(&runtime, before)?;
                    let scheduled = house_events::scheduled_houses(&runtime, after.tick)?;
                    runtime.advance_world(1)?;
                    house_events::loop_equal(&runtime, after)?;
                    let start = hp;
                    while houses.get(hp).is_some_and(|e| e.call == calls) {
                        hp += 1;
                    }
                    let _house_draws = house_events::houses_equal(
                        &runtime,
                        houses.get(start..hp).ok_or("house range")?,
                        &scheduled,
                        calls,
                        after.tick,
                    )?;
                    let start = ip;
                    if before.tick != after.tick {
                        loop {
                            let event = industries
                                .get(ip)
                                .ok_or("missing post-station industry phase")?;
                            ip += 1;
                            if event.phase == "production" && event.edge == "leave" {
                                break;
                            }
                        }
                    }
                    draws += observe::call(
                        &runtime,
                        industries.get(start..ip).ok_or("industry range")?,
                        after.tick,
                    )?;
                }
            }
            Action::Checkpoint { ordinal, label } => {
                assert_eq!(usize::try_from(ordinal)?, index);
                checkpoint(&runtime, &native, &output, &label)?;
                boundaries += 1;
            }
        }
    }
    assert!(loops.next().is_none());
    assert_eq!(hp, houses.len());
    assert_eq!(ip, industries.len());
    checkpoint(&runtime, &native, &output, "final")?;
    write(
        &output.join("PASS.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "calls":calls,"complete_world_derived_town_industry_boundaries":boundaries,
            "industry_event_edges":industries.len(),"tile_and_production_random_draws":draws,
            "house_event_edges":houses.len(),"complete_loop_rng_cursor_script_each_call":true,
        }))?,
    )
}

#[test]
#[ignore = "requires the original mixed road/industry observer corpus"]
fn native_mixed_industry_matches_complete_runtime_caches() -> Result {
    let native = PathBuf::from(std::env::var("OTTD_INDUSTRY_NATIVE_DIR")?);
    let output = PathBuf::from(std::env::var("OTTD_INDUSTRY_RUST_OUTPUT")?);
    std::fs::create_dir(&output)?;
    let protocol: serde_json::Value =
        serde_json::from_reader(File::open(native.join("protocol.json"))?)?;
    let calls = protocol
        .get("calls")
        .and_then(serde_json::Value::as_u64)
        .ok_or("movement calls")?;
    let world = World::decode(&Savegame::decode(
        &std::fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    checkpoint(&runtime, &native, &output, "initial")?;
    let industries = house_events::read::<observe::Event>(&native.join("industry-events.jsonl"))?;
    let mut position = 0;
    let mut draws = 0_u64;
    for call in 1..=calls {
        runtime.advance_world(1)?;
        let start = position;
        loop {
            let event = industries.get(position).ok_or("mixed production phase")?;
            position += 1;
            if event.phase == "production" && event.edge == "leave" {
                break;
            }
        }
        let tick = house_events::number(runtime.world(), *b"DATE", 0, "tick_counter")?;
        draws += observe::call(
            &runtime,
            industries.get(start..position).ok_or("mixed event range")?,
            tick,
        )?;
        checkpoint(&runtime, &native, &output, &format!("tick_{call}"))?;
    }
    assert_eq!(position, industries.len());
    checkpoint(&runtime, &native, &output, "final")?;
    write(
        &output.join("PASS.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "calls":calls,"complete_world_derived_town_industry_boundaries":calls + 2,
            "industry_event_edges":industries.len(),"tile_and_production_random_draws":draws,
        }))?,
    )
}
