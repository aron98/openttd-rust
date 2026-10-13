//! External original-vs-Rust station command and saved continuation runner.
use super::*;
use crate::{
    CommandRequest,
    runtime::{RuntimeSaveContext, SimulationRuntime},
};
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
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Command { request: CommandRequest },
    Save { label: String },
}
fn observe(runtime: &SimulationRuntime) -> Result<Value> {
    let world = runtime.world();
    let mut stops = Vec::new();
    for id in world.tables().get(b"ROAD").ok_or("ROAD")?.records().keys() {
        let row = saved::row(world, *b"ROAD", *id)?;
        stops.push(json!({"id":id,"xy":row.number("xy")?,"status":row.number("status")?,"next":row.number("next")?.checked_sub(1)}));
    }
    let road: BTreeMap<_, _> = runtime
        .road_infrastructure()
        .iter()
        .map(|(id, pieces)| (*id, pieces.to_vec()))
        .collect();
    let mut units = Vec::new();
    for id in world.tables().get(b"PLYR").ok_or("PLYR")?.records().keys() {
        let id = u8::try_from(*id)?;
        let next = runtime
            .allocation
            .freeunits
            .get(&(id, crate::runtime::pools::VehicleType::Road))
            .map_or(1, crate::runtime::pools::UnitNumberAllocator::next_id);
        units.push(
            json!({"company":id,"next":next,"count":crate::runtime::road_company_count(world,id)?}),
        );
    }
    let vehicles = json!({"road":runtime.road.values().collect::<Vec<_>>(),"pool":runtime.allocation.pool.snapshot(),"units":units});
    Ok(
        json!({"orders":runtime.order_state_json()?,"depot_pool":runtime.depot_pool(),
        "sale":{"vehicle":vehicles,"groups":runtime.road_group_counts()?},"road_infrastructure":road,"station_pool":runtime.station_pool(),"road_stop_pool":runtime.road_stop_pool(),
        "geometry":runtime.road_station_geometry(),"infrastructure":runtime.station_infrastructure(),"stops":stops,
        "random":[crate::world_access::unsigned(world,b"DATE",0,"random_state[0]")?,crate::world_access::unsigned(world,b"DATE",0,"random_state[1]")?]}),
    )
}
fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_'))
}
#[test]
#[ignore = "ROAD_STATION_INPUT/ACTIONS/OUTPUT; actual native station command differential"]
fn original_road_station_case() -> Result {
    let source = PathBuf::from(std::env::var("ROAD_STATION_INPUT")?);
    let descriptor = PathBuf::from(std::env::var("ROAD_STATION_ACTIONS")?);
    let output = PathBuf::from(std::env::var("ROAD_STATION_OUTPUT")?);
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
        return Err("invalid station descriptor".into());
    }
    let world = World::decode(&Savegame::decode(
        &std::fs::read(source)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let (mut runtime, _) =
        SimulationRuntime::from_loaded_vanilla(world, RuntimeSaveContext::SinglePlayer)?;
    std::fs::create_dir(&output)?;
    let initial = observe(&runtime)?;
    std::fs::write(
        output.join("initial.world.json"),
        serde_json::to_vec(&runtime.saved_json()?)?,
    )?;
    let mut actions = Vec::new();
    for (index, action) in plan.actions.iter().enumerate() {
        let before = observe(&runtime)?;
        let result = match action {
            Action::Command { request } => json!({"receipt":runtime.execute_command(request)?}),
            Action::Save { label } => {
                if !valid_label(label) {
                    return Err("invalid label".into());
                }
                let path = format!("{label}.sav");
                let mut file = std::fs::File::create_new(output.join(&path))?;
                std::io::Write::write_all(
                    &mut file,
                    &runtime.to_savegame()?.encode(Compression::None)?,
                )?;
                std::fs::write(
                    output.join(format!("{label}.world.json")),
                    serde_json::to_vec(&runtime.saved_json()?)?,
                )?;
                std::fs::write(
                    output.join(format!("{label}.derived.json")),
                    serde_json::to_vec(runtime.world().derived())?,
                )?;
                json!({"path":path,"before":before,"after":observe(&runtime)?})
            }
        };
        actions.push(json!({"index":index,"input":action,"before":before,"result":result,"after":observe(&runtime)?}));
    }
    std::fs::write(
        output.join("results.json"),
        serde_json::to_vec(&json!({"schema_version":1,"case":plan.case,
        "initial":initial,"actions":actions,"final":observe(&runtime)?}))?,
    )?;
    println!("PASS road station commands: {} actions", plan.actions.len());
    Ok(())
}
