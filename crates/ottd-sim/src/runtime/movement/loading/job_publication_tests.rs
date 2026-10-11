//! Synthetic publication controls on a genuine saved job; no native parity claim.
use super::job_publication::Publication;
use crate::{WorldTickState, runtime::SimulationRuntime};
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use std::path::PathBuf;

#[test]
#[ignore = "requires genuine original prejoin save; explicitly synthetic cache/identity controls"]
fn synthetic_goods_publication_predicates() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(std::env::var("JOB_INPUT")?)?;
    let output = PathBuf::from(std::env::var("JOB_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let mut rows = Vec::new();
    for mode in [
        "clear",
        "reserved",
        "graph-gone",
        "station-gone",
        "identity",
        "waypoint",
    ] {
        let mut runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
            &bytes,
            ottd_save::DEFAULT_MAX_BYTES,
        )?)?)?;
        let mut input = runtime
            .transport
            .job_inputs
            .get(&0)
            .and_then(Clone::clone)
            .ok_or("job input")?;
        let result = runtime.transport.workers.get(&0).ok_or("worker")?.wait()?;
        let committed = runtime.transport.cargo.clone();
        let world_before = runtime.world.saved_json()?;
        let mut candidate = committed.clone();
        let entries = candidate.stations.get_mut(&0).ok_or("station0")?;
        let empty = entries.entry(0).or_default();
        assert_eq!(
            empty.count, 0,
            "original prejoin station0 has no waiting packets"
        );
        if mode == "reserved" {
            empty.reserved = 1;
        }
        if mode == "graph-gone" {
            input.graph = 65534;
        }
        if mode == "station-gone" {
            input.nodes.last_mut().ok_or("node")?.station = 64000;
        }
        let mut state = WorldTickState::load(&mut runtime.world, None);
        if mode == "identity" {
            super::records::goods_field(&mut state, (0, 0), ("node", WireValue::Unsigned(19)))?;
        }
        if mode == "waypoint" {
            state.apply(WorldEdit::Field {
                chunk: *b"STNN",
                record: 0,
                path: vec![PathElement::Field("facilities".into())],
                value: WireValue::Unsigned(128),
            })?;
        }
        let populated = candidate.stations.get(&1).ok_or("station1")?.clone();
        result.publish(
            &input,
            &mut Publication {
                state: &mut state,
                cargo: &mut candidate,
            },
        )?;
        assert_eq!(candidate.stations.get(&1), Some(&populated));
        let retained = candidate
            .stations
            .get(&0)
            .ok_or("station0")?
            .contains_key(&0);
        std::fs::write(
            output.join(format!("{mode}.json")),
            serde_json::to_vec_pretty(&candidate.snapshot())?,
        )?;
        assert_eq!(retained, mode != "clear", "{mode}");
        if mode == "clear" {
            // A labelled late pool fault follows the observed positive cache deletion.
            candidate.pool.allocate()?;
            assert!(candidate.validate(state.view()).is_err());
        }
        drop(state);
        assert_eq!(runtime.world.saved_json()?, world_before);
        assert_eq!(runtime.transport.cargo, committed);
        rows.push(serde_json::json!({"mode":mode,"synthetic":true,"populated_station_preserved":true,"committed_world_cache_unchanged":true}));
    }
    std::fs::write(output.join("PASS.json"), serde_json::to_vec_pretty(&rows)?)?;
    Ok(())
}
