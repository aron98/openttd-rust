//! Bounded events and complete boundaries from actual original whole-world house loops.
use ottd_save::{Compression, Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde::Deserialize;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
#[path = "native_house_lifecycle/events.rs"]
mod events;
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
    let mut output = File::create_new(path)?;
    output.write_all(bytes)?;
    output.sync_all()?;
    Ok(())
}

fn catalog(native: &Path) -> Result {
    let specs: Vec<serde_json::Value> =
        serde_json::from_reader(File::open(native.join("initial.house-specs.json"))?)?;
    assert_eq!(specs.len(), ottd_sim::content::HouseSpec::vanilla().len());
    for (id, (native, rust)) in specs
        .iter()
        .zip(ottd_sim::content::HouseSpec::vanilla())
        .enumerate()
    {
        assert_eq!(native["id"].as_u64(), Some(u64::try_from(id)?));
        for (field, value) in [
            ("population", rust.population),
            ("mail_generation", rust.mail_generation),
            ("building_flags", rust.building_flags),
        ] {
            assert_eq!(
                native[field].as_u64(),
                Some(u64::from(value)),
                "house {id}/{field}"
            );
        }
    }
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
        let expected: serde_json::Value = serde_json::from_reader(File::open(native.join(&name))?)?;
        assert_eq!(actual, expected, "{name}");
    }
    write(
        &output.join(format!("{label}.sav")),
        &runtime.to_savegame()?.encode(Compression::Zlib)?,
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original house lifecycle events and a fresh output directory"]
fn native_house_lifecycle_matches_complete_runtime() -> Result {
    let native = PathBuf::from(std::env::var("OTTD_HOUSE_NATIVE_DIR")?);
    let output = PathBuf::from(std::env::var("OTTD_HOUSE_RUST_OUTPUT")?);
    std::fs::create_dir(&output)?;
    catalog(&native)?;
    let protocol: Protocol = serde_json::from_reader(File::open(native.join("protocol.json"))?)?;
    assert_eq!(protocol.schema_version, 1);
    let world = World::decode(&Savegame::decode(
        &std::fs::read(native.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    checkpoint(&runtime, &native, &output, "initial")?;
    let loops = events::read::<events::LoopEvent>(&native.join("world-events.jsonl"))?;
    let houses = events::read::<events::HouseEvent>(&native.join("events.jsonl"))?;
    let mut loop_events = loops.iter();
    let mut house_position = 0;
    let mut call = 0_u32;
    let mut draws = 0_u64;
    let mut checkpoints = 2_u32;
    for (index, action) in protocol.actions.into_iter().enumerate() {
        match action {
            Action::Tick { ordinal, count } => {
                assert_eq!(usize::try_from(ordinal)?, index);
                if count == 0 {
                    assert!(runtime.advance_world(0)?.ticks.is_empty());
                }
                for _ in 0..count {
                    call = call.checked_add(1).ok_or("call overflow")?;
                    let before = loop_events.next().ok_or("loop enter event")?;
                    let after = loop_events.next().ok_or("loop leave event")?;
                    assert_eq!(before.edge, "enter");
                    assert_eq!(after.edge, "leave");
                    assert_eq!((before.call, after.call), (call, call));
                    assert_eq!(before.sequence.checked_add(1), Some(after.sequence));
                    assert_eq!(before.interactive_random, after.interactive_random);
                    events::loop_equal(&runtime, before)?;
                    let scheduled = events::scheduled_houses(&runtime, after.tick)?;
                    runtime.advance_world(1)?;
                    events::loop_equal(&runtime, after)?;
                    let start = house_position;
                    while houses
                        .get(house_position)
                        .is_some_and(|event| event.call == call)
                    {
                        house_position = house_position
                            .checked_add(1)
                            .ok_or("house position overflow")?;
                    }
                    draws = draws
                        .checked_add(events::houses_equal(
                            &runtime,
                            houses
                                .get(start..house_position)
                                .ok_or("house event range")?,
                            &scheduled,
                            call,
                            after.tick,
                        )?)
                        .ok_or("draw total")?;
                }
            }
            Action::Checkpoint { ordinal, label } => {
                assert_eq!(usize::try_from(ordinal)?, index);
                checkpoint(&runtime, &native, &output, &label)?;
                checkpoints = checkpoints.checked_add(1).ok_or("checkpoint overflow")?;
            }
        }
    }
    assert!(loop_events.next().is_none());
    assert_eq!(house_position, houses.len());
    checkpoint(&runtime, &native, &output, "final")?;
    write(
        &output.join("PASS.json"),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "calls": call, "house_events": houses.len(), "loop_events": loops.len(),
            "native_house_random_draws": draws,
            "complete_world_derived_cache_boundaries": checkpoints,
            "rng_cursor_counter_and_house_mutations_each_complete_call": true,
        }))?,
    )?;
    Ok(())
}
