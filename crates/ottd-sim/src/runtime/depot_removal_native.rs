//! Private native command proof dispatcher, with setup performed by original APIs.
use super::*;
use crate::CommandRequest;
use ottd_save::{Compression, Savegame};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema_version: u32,
    case: String,
    role: String,
    exit: bool,
    actions: Vec<Action>,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Command { request: CommandRequest },
    Backup { vehicle: u32, user: u32 },
    Save { label: String },
    Snapshot { label: String },
}
pub(super) fn observe(runtime: &SimulationRuntime) -> Result<Value> {
    let mut units = Vec::new();
    for owner in runtime
        .world
        .tables()
        .get(b"PLYR")
        .ok_or("PLYR")?
        .records()
        .keys()
    {
        let owner = u8::try_from(*owner)?;
        units.push(json!({"company":owner,"next":runtime.allocation.road_units.get(&owner).map_or(1,pools::UnitNumberAllocator::next_id),"count":road_company_count(&runtime.world,owner)?}));
    }
    let road: BTreeMap<_, _> = runtime
        .depot
        .road
        .iter()
        .map(|(id, counts)| (id.to_string(), counts.as_slice()))
        .collect();
    let tiles: Vec<_> = runtime
        .world
        .map()
        .tiles()
        .iter()
        .map(|t| {
            json!([
                t.tile_type(),
                t.height(),
                t.m1(),
                t.m2(),
                t.m3(),
                t.m4(),
                t.m5(),
                t.m6(),
                t.m7(),
                t.m8()
            ])
        })
        .collect();
    Ok(
        json!({"orders":runtime.order_state_json()?,"depot":{"pool":runtime.depot.pool.snapshot(),"road":road},"vehicles":{"road":runtime.road.values().collect::<Vec<_>>(),"pool":runtime.allocation.pool.snapshot(),"units":units},"tiles":tiles}),
    )
}
fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_'))
}
#[test]
#[ignore = "DEPOT_REMOVAL_INPUT/ACTIONS/OUTPUT; actual original command differential"]
fn original_depot_removal_case() -> Result {
    let input = PathBuf::from(std::env::var("DEPOT_REMOVAL_INPUT")?);
    let descriptor = PathBuf::from(std::env::var("DEPOT_REMOVAL_ACTIONS")?);
    let output = PathBuf::from(std::env::var("DEPOT_REMOVAL_OUTPUT")?);
    let bytes = std::fs::read(descriptor)?;
    if bytes.len() > 262_144 {
        return Err("descriptor too large".into());
    }
    let plan: Plan = serde_json::from_slice(&bytes)?;
    if plan.schema_version != 1
        || plan.role != "sp"
        || !plan.exit
        || !valid_label(&plan.case)
        || plan.actions.is_empty()
        || plan.actions.len() > 256
    {
        return Err("invalid command proof descriptor".into());
    }
    let world = World::decode(&Savegame::decode(
        &std::fs::read(input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let (mut runtime, _) =
        SimulationRuntime::from_loaded_vanilla(world, RuntimeSaveContext::SinglePlayer)?;
    std::fs::create_dir(&output)?;
    let initial = observe(&runtime)?;
    let mut actions = Vec::new();
    for (index, action) in plan.actions.iter().enumerate() {
        let before = observe(&runtime)?;
        let result = match action {
            Action::Command { request } => json!({"receipt":runtime.execute_command(request)?}),
            Action::Backup { vehicle, user } => {
                runtime.backup_orders(VehicleId::new(*vehicle), *user)?;
                Value::Null
            }
            Action::Snapshot { label } => {
                if !valid_label(label) {
                    return Err("invalid label".into());
                }
                Value::Null
            }
            Action::Save { label } => {
                if !valid_label(label) {
                    return Err("invalid label".into());
                }
                let path = format!("{label}.sav");
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(output.join(&path))?;
                std::io::Write::write_all(
                    &mut file,
                    &runtime.to_savegame()?.encode(Compression::None)?,
                )?;
                json!({"path":path,"before":before,"after":observe(&runtime)?})
            }
        };
        actions.push(json!({"index":index,"input":action,"before":before,"result":result,"after":observe(&runtime)?}));
    }
    std::fs::write(
        output.join("results.json"),
        serde_json::to_vec(
            &json!({"schema_version":1,"case":plan.case,"initial":initial,"actions":actions,"final":observe(&runtime)?}),
        )?,
    )?;
    println!("PASS depot removal: {} actions", plan.actions.len());
    Ok(())
}
