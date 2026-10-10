use super::occupied;
use crate::commands::CommandCost;
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
struct Observation {
    runtime: Value,
    vehicles: Value,
    occupancy: Option<NativeWitness>,
}

#[derive(Debug, Deserialize)]
struct NativeWitness {
    before: Value,
    after: Value,
    vectors: [Vector; 2],
    road_error_id: u16,
    invalid_error_id: u16,
    hash_restored: HashRestored,
    members_before: Vec<u32>,
    members_after: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct HashRestored {
    current: bool,
    previous: bool,
    next: bool,
}

#[derive(Debug, Deserialize)]
struct Vector {
    vehicle: u32,
    tile: u32,
    z: i64,
    maximum_z: i64,
    #[serde(flatten)]
    result: Outcome,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Outcome {
    success: bool,
    error_id: u16,
    cost: i64,
    expenses: u8,
}

fn world(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}

fn height(vehicle: u32, z: i64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: vehicle,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field("z_pos".into()),
        ],
        value: WireValue::Signed(z),
    }
}

#[test]
#[ignore = "fresh original occupancy vectors and independent canonical reload"]
fn native_ground_cutoff() -> Result {
    let root = std::path::PathBuf::from(std::env::var("DEPOT_OCCUPANCY_CASE")?);
    let canonical: Observation =
        serde_json::from_slice(&std::fs::read(root.join("canonical/depot-runtime.json"))?)?;
    let observed: Observation =
        serde_json::from_slice(&std::fs::read(root.join("vectors/depot-runtime.json"))?)?;
    assert!(canonical.occupancy.is_none());
    let vectors = observed.occupancy.ok_or("missing native vectors")?;
    let expected_live = json!({"depot": canonical.runtime, "vehicles": canonical.vehicles});
    assert_eq!(vectors.before, expected_live);
    assert_eq!(vectors.after, expected_live);
    assert_eq!(
        json!({"depot": observed.runtime, "vehicles": observed.vehicles}),
        expected_live
    );
    let canonical_world = world(&root.join("canonical/save/autosave/exit.sav"))?;
    let canonical_saved = canonical_world.saved_json()?;
    assert_eq!(
        world(&root.join("vectors/save/autosave/exit.sav"))?.saved_json()?,
        canonical_saved,
        "native occupancy vectors leaked into the complete saved world"
    );
    assert_eq!(
        world(&root.join("reload/save/autosave/exit.sav"))?.saved_json()?,
        canonical_saved,
        "native restored state changed after independent reload"
    );
    let reloaded: Observation =
        serde_json::from_slice(&std::fs::read(root.join("reload/depot-runtime.json"))?)?;
    assert_eq!(
        json!({"depot": reloaded.runtime, "vehicles": reloaded.vehicles}),
        expected_live
    );
    let [at_maximum, above] = &vectors.vectors;
    assert_eq!(at_maximum.z, at_maximum.maximum_z);
    assert_eq!(
        above.z,
        above.maximum_z.checked_add(1).ok_or("maximum overflow")?
    );
    assert_eq!(at_maximum.maximum_z, above.maximum_z);
    assert_eq!(at_maximum.vehicle, above.vehicle);
    assert_eq!(at_maximum.tile, above.tile);
    assert!(vectors.hash_restored.current);
    assert!(vectors.hash_restored.previous);
    assert!(vectors.hash_restored.next);
    assert_eq!(vectors.members_before, vec![at_maximum.vehicle]);
    assert_eq!(vectors.members_after, vectors.members_before);
    assert_ne!(vectors.road_error_id, vectors.invalid_error_id);
    let mut compared = Vec::new();
    for vector in &vectors.vectors {
        let mut candidate = canonical_world.clone();
        candidate.edit_batch(vec![height(vector.vehicle, vector.z)])?;
        let cost =
            occupied(&candidate, vector.tile)?.unwrap_or_else(|| CommandCost::success(0, 255));
        let error_id = match cost.error.as_deref() {
            None => vectors.invalid_error_id,
            Some("STR_ERROR_ROAD_VEHICLE_IN_THE_WAY") => vectors.road_error_id,
            Some(_) => return Err("unexpected pure occupancy error".into()),
        };
        let result = Outcome {
            success: cost.success,
            error_id,
            cost: cost.cost,
            expenses: cost.expenses,
        };
        assert_eq!(
            result, vector.result,
            "pure occupancy result differs from original"
        );
        compared.push(result);
    }
    assert!(!at_maximum.result.success);
    assert!(above.result.success);
    assert_eq!(canonical_world.saved_json()?, canonical_saved);
    std::fs::write(
        root.join("rust-vectors.json"),
        serde_json::to_vec_pretty(&compared)?,
    )?;
    Ok(())
}
