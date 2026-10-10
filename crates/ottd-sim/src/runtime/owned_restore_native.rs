//! Strict loaded-input owned Restore differential dispatcher.
use super::*;
use crate::{Command, CommandRequest};
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
    BackupUsers { vehicle: u32, users: Vec<u32> },
    Save { label: String },
    Snapshot { label: String },
}
use super::ordered_sale_native::observe;
fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_'))
}
#[test]
#[ignore = "OWNED_RESTORE_INPUT/ACTIONS/OUTPUT; actual original command differential"]
fn original_owned_restore_case() -> Result {
    let input = PathBuf::from(std::env::var("OWNED_RESTORE_INPUT")?);
    let descriptor = PathBuf::from(std::env::var("OWNED_RESTORE_ACTIONS")?);
    let output = PathBuf::from(std::env::var("OWNED_RESTORE_OUTPUT")?);
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
    std::fs::write(
        output.join("initial.world.json"),
        serde_json::to_vec(&runtime.saved_json()?)?,
    )?;
    let mut actions = Vec::new();
    for (index, action) in plan.actions.iter().enumerate() {
        let before = observe(&runtime)?;
        let result = match action {
            Action::Command { request } => {
                if !matches!(
                    request.command,
                    Command::BuildVehicle { .. }
                        | Command::SellVehicle { .. }
                        | Command::LandscapeClear { .. }
                ) {
                    return Err("ordered-sale dispatcher command boundary".into());
                }
                json!({"receipt":runtime.execute_command(request)?})
            }
            Action::Backup { vehicle, user } => {
                runtime.backup_orders(VehicleId::new(*vehicle), *user)?;
                Value::Null
            }
            Action::BackupUsers { vehicle, users } => {
                if users.len() > 256 {
                    return Err("invalid bounded backup users".into());
                }
                let mut states = Vec::new();
                for user in users {
                    runtime.backup_orders(VehicleId::new(*vehicle), *user)?;
                    states.push(json!({"user":user,"pool":runtime.order_backup_pool()}));
                }
                json!(states)
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
                std::fs::write(
                    output.join(format!("{label}.world.json")),
                    serde_json::to_vec(&runtime.saved_json()?)?,
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
    println!("PASS owned Restore: {} actions", plan.actions.len());
    Ok(())
}
