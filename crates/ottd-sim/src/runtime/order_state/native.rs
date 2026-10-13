//! Actual production order primitive dispatcher; fixture setup belongs to original APIs.
use super::*;
use ottd_save::Compression;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema_version: u32,
    case: String,
    role: Role,
    exit: bool,
    actions: Vec<Action>,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    Sp,
    Server,
    Client,
}
impl Role {
    const fn context(self) -> RuntimeSaveContext {
        match self {
            Self::Sp => RuntimeSaveContext::SinglePlayer,
            Self::Server => RuntimeSaveContext::NetworkServer,
            Self::Client => RuntimeSaveContext::NetworkClient,
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Backup {
        vehicle: u32,
        user: u32,
    },
    BackupUsers {
        vehicle: u32,
        users: Vec<u32>,
    },
    ResetUser {
        user: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tile: Option<u32>,
    },
    ResetTile {
        tile: u32,
    },
    ClearGroup {
        group: u16,
    },
    ClearVehicle {
        vehicle: u32,
    },
    InvalidateDepot {
        depot_id: u16,
        tile: u32,
    },
    Snapshot {
        label: String,
    },
    Save {
        label: String,
    },
    AwaitFile {
        label: String,
    },
}
fn label(value: &str) -> Result {
    if value.is_empty()
        || !value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || v == b'-' || v == b'_')
    {
        return Err("invalid label".into());
    }
    Ok(())
}
fn save(state: &OrderState, world: &World, path: &Path) -> Result {
    let saved = match state.context {
        RuntimeSaveContext::NetworkServer => world.to_savegame()?,
        RuntimeSaveContext::SinglePlayer | RuntimeSaveContext::NetworkClient => {
            world.without_order_backups().to_savegame()?
        }
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    std::io::Write::write_all(&mut file, &saved.encode(Compression::None)?)?;
    Ok(())
}
fn action(
    state: &mut OrderState,
    world: &mut World,
    action: &Action,
    output: &Path,
) -> Result<Value> {
    let planned = match action {
        Action::Backup { vehicle, user } => Some(backup::create(
            state,
            world,
            VehicleId::new(*vehicle),
            *user,
        )?),
        Action::ResetUser { user, tile } => Some(backup::reset(
            state,
            world,
            BackupReset::User {
                user: *user,
                tile: *tile,
            },
        )?),
        Action::ResetTile { tile } => {
            Some(backup::reset(state, world, BackupReset::AtTile(*tile))?)
        }
        Action::ClearGroup { group } => Some(backup::clear_group(state, world, *group)?),
        Action::ClearVehicle { vehicle } => Some(backup::clear_vehicle(
            state,
            world,
            VehicleId::new(*vehicle),
        )?),
        Action::BackupUsers { vehicle, users } => {
            if users.is_empty() || users.len() > 256 {
                return Err("invalid backup users count".into());
            }
            let mut states = Vec::new();
            for user in users {
                let (edits, pool) = backup::create(state, world, VehicleId::new(*vehicle), *user)?;
                state.publish(world, edits, pool)?;
                states.push(json!({"user":user,"pool":state.backups.snapshot()}));
            }
            return Ok(json!(states));
        }
        Action::InvalidateDepot { depot_id, tile } => {
            let plan = state.plan_depot_invalidation(
                OrderReader::Committed(world),
                &world.derived().order_lists,
                *depot_id,
                *tile,
                world.map().width(),
                world.map().height(),
            )?;
            let mut tx = world.transaction();
            let pending = plan.stage(&mut tx)?;
            let (prepared, delta) = pending.validate(tx.prepare()?)?;
            prepared.commit();
            delta.publish(state);
            None
        }
        Action::Save { label: name } => {
            label(name)?;
            let before = state.observe(world)?;
            let filename = format!("{name}.sav");
            save(state, world, &output.join(&filename))?;
            return Ok(json!({"path":filename,"before":before,"after":state.observe(world)?}));
        }
        Action::Snapshot { label: name } | Action::AwaitFile { label: name } => {
            label(name)?;
            None
        }
    };
    if let Some((edits, pool)) = planned {
        state.publish(world, edits, pool)?;
    }
    Ok(Value::Null)
}
#[test]
#[ignore = "ORDER_CASE_INPUT, ORDER_CASE_ACTIONS and fresh ORDER_CASE_OUTPUT; actual native paired proof"]
fn original_order_state_case() -> Result {
    let input = PathBuf::from(std::env::var("ORDER_CASE_INPUT")?);
    let descriptor = PathBuf::from(std::env::var("ORDER_CASE_ACTIONS")?);
    let output = PathBuf::from(std::env::var("ORDER_CASE_OUTPUT")?);
    let mut bytes = Vec::new();
    std::fs::File::open(&descriptor)?
        .take(262_145)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 262_144 {
        return Err("action descriptor exceeds262144bytes".into());
    }
    let plan: Plan = serde_json::from_slice(&bytes)?;
    if plan.schema_version != 1 || plan.actions.is_empty() || plan.actions.len() > 4096 {
        return Err("invalid exact action inventory".into());
    }
    label(&plan.case)?;
    if output.exists() {
        return Err("output already exists".into());
    }
    let mut world = World::decode(&ottd_save::Savegame::decode(
        &std::fs::read(&input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let _content = crate::content::ContentCatalog::from_world(&world)?;
    let (mut state, receipt) = OrderState::from_loaded(&mut world, plan.role.context())?;
    std::fs::create_dir_all(&output)?;
    let initial = state.observe(&world)?;
    let mut actions = Vec::new();
    for (index, a) in plan.actions.iter().enumerate() {
        let before = state.observe(&world)?;
        let result = action(&mut state, &mut world, a, &output)?;
        actions.push(json!({"index":index,"input":a,"before":before,"result":result,"after":state.observe(&world)?}));
    }
    let results = json!({"schema_version":1,"case":plan.case,"declared_context":plan.role,"exit_requested":plan.exit,"load_receipt":receipt,"initial":initial,"actions":actions,"final":state.observe(&world)?});
    std::fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(&results)?,
    )?;
    println!("PASS order state case: {} actions", plan.actions.len());
    Ok(())
}

#[test]
#[ignore = "ORDER_CASE_INPUT and fresh ORDER_AIRPORT_OUTPUT; exact40 original geometry comparisons"]
fn original_airport_geometry() -> Result {
    let input = std::env::var("ORDER_CASE_INPUT")?;
    let output = PathBuf::from(std::env::var("ORDER_AIRPORT_OUTPUT")?);
    if output.exists() {
        return Err("airport output already exists".into());
    }
    let world = World::decode(&ottd_save::Savegame::decode(
        &std::fs::read(input)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let _content = crate::content::ContentCatalog::from_world(&world)?;
    let width = world.map().width();
    let base = width
        .checked_mul(8)
        .and_then(|v| v.checked_add(8))
        .ok_or("geometry origin")?;
    let mut rows = Vec::new();
    for kind in 0..10 {
        let spec = airport::layout(kind)?;
        for rotation in [0, 2, 4, 6] {
            let tiles = airport::hangar_tiles(kind, rotation, base, width, world.map().height())?;
            let depots: Vec<_> = spec
                .depots
                .iter()
                .zip(tiles)
                .enumerate()
                .map(
                    |(number, ((x, y), tile))| json!({"x":x,"y":y,"hangar_num":number,"tile":tile}),
                )
                .collect();
            rows.push(json!({"type":kind,"rotation":rotation,"base":base,"map_width":width,"size_x":spec.width,"size_y":spec.height,"depots":depots}));
        }
    }
    if rows.len() != 40 {
        return Err("airport geometry count".into());
    }
    let value = json!({"scope":"pure-original-offset-query-not-placed-airport","rows":rows});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    std::io::Write::write_all(&mut file, &serde_json::to_vec_pretty(&value)?)?;
    println!("PASS airport geometry: 40 rows");
    Ok(())
}
