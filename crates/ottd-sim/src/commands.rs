//! Native top-level command phases over the authoritative saved world.
mod cargo_capacities;
mod finance;
mod vehicle_build;
mod vehicle_sale;
pub use cargo_capacities::CargoCapacities;
mod landscape;
mod level_land;
mod naming;
mod occupancy;
mod pause;
mod pipeline;
mod road;
mod road_depot;
mod road_vehicle;
mod terraform;
mod terrain_read;

use crate::world_access::WorldAccessError;
use ottd_save::world::{World, WorldEdit, WorldError};
pub use pipeline::execute_command;
use serde::{Deserialize, Serialize};

/// Local native posting mode; pause commands have the native `NoEst` trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandMode {
    /// Validate and execute.
    Post,
    /// Estimate without an affordability check or mutation, except `NoEst` commands.
    Estimate,
}
/// Supported native command arguments. Unsupported gameplay is a separate error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Build or rotate a vanilla road depot.
    BuildRoadDepot {
        /// Map tile.
        tile: u32,
        /// Native road type.
        road_type: u8,
        /// Native entrance direction.
        direction: u8,
    },
    /// Sell an admitted single-part vanilla road vehicle.
    SellVehicle {
        /// Native Post feedback location.
        location: u32,
        /// Vehicle pool ID.
        vehicle: u32,
        /// Native flag; no effect for single-part road vehicles.
        sell_chain: bool,
        /// Preserve orders for a later purchase; currently unsupported.
        backup_order: bool,
        /// Native order-backup client identity.
        client_id: u32,
    },
    /// Buy one admitted vanilla road vehicle in an existing depot.
    BuildVehicle {
        /// Native depot tile.
        tile: u32,
        /// Native engine ID.
        engine: u16,
        /// Requested cargo or 255 for the default.
        cargo: u8,
        /// Native flag, with no effect for road vehicles.
        use_free_vehicles: bool,
        /// Native order-backup client identity; zero is the local server.
        client_id: u32,
    },
    /// Change a primary road vehicle's automatic service interval.
    ChangeServiceInterval {
        /// Native vehicle pool index.
        vehicle: u32,
        /// Requested interval in days, minutes or percent.
        interval: u16,
        /// Use this interval instead of the company default.
        custom: bool,
        /// Interpret a custom interval as a reliability percentage.
        percent: bool,
    },
    /// Level a native rectangular or diagonal selection with partial completion.
    LevelLand {
        /// End tile of the selection, including native void tiles.
        tile: u32,
        /// Start tile whose initial height determines the target.
        start_tile: u32,
        /// Use native diagonal iteration instead of a rectangle.
        diagonal: bool,
        /// Native raw mode: level zero, lower one, raise two.
        level_mode: u8,
    },
    /// Change selected terrain corners by one height level.
    TerraformLand {
        /// Linear tile index, including native void tiles.
        tile: u32,
        /// Native raw slope byte; only its four corner bits select work.
        slope: u8,
        /// Raise if true, lower otherwise.
        dir_up: bool,
    },
    /// Build vanilla road pieces on supported terrain.
    BuildRoad {
        /// Linear tile index.
        tile: u32,
        /// Four native road direction bits.
        pieces: u8,
        /// Native road type; this stage supports vanilla road zero.
        road_type: u8,
        /// Native one-way direction mask to toggle.
        toggle_disallowed: u8,
        /// Native town ID; company builders must supply 65535.
        town_id: u16,
    },
    /// Clear a supported clear-ground tile.
    LandscapeClear {
        /// Linear tile index.
        tile: u32,
    },
    /// Borrow money directly into company cash.
    IncreaseLoan {
        /// Native method: interval zero, maximum one, amount two.
        method: u8,
        /// Explicit amount for method two.
        amount: i64,
    },
    /// Repay a company loan.
    DecreaseLoan {
        /// Native method: interval zero, maximum one, amount two.
        method: u8,
        /// Explicit amount for method two.
        amount: i64,
    },
    /// Rename the company or reset its name with an empty string.
    RenameCompany {
        /// UTF-8 name, fewer than 32 characters.
        text: String,
    },
    /// Rename the president, including native nested company naming.
    RenamePresident {
        /// UTF-8 name, fewer than 32 characters.
        text: String,
    },
    /// Set one native pause reason.
    Pause {
        /// Native bit index, not a mask.
        mode: u8,
        /// Set or clear the reason.
        paused: bool,
    },
}
/// One local command invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequest {
    /// Acting company. Server commands execute as spectator internally.
    pub company: u8,
    /// Local post or estimate semantics.
    pub mode: CommandMode,
    /// Native command arguments.
    pub command: Command,
}
/// Native cost result, distinct from rejection before a command runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandCost {
    /// Whether native validation/execution succeeded.
    pub success: bool,
    /// Signed native cost, retained even after affordability failure.
    pub cost: i64,
    /// Native expense category, including 255 for unspecified.
    pub expenses: u8,
    /// Native string symbol, or `CMD_ERROR` for generic failure.
    pub error: Option<String>,
    /// Integer parameters of the native encoded error.
    pub error_params: Vec<i64>,
}
impl CommandCost {
    pub(crate) const fn success(cost: i64, expenses: u8) -> Self {
        Self {
            success: true,
            cost,
            expenses,
            error: None,
            error_params: Vec::new(),
        }
    }
    pub(crate) fn failure(symbol: &str) -> Self {
        Self {
            success: false,
            cost: 0,
            expenses: 255,
            error: Some(symbol.to_owned()),
            error_params: Vec::new(),
        }
    }
    fn parameterized(symbol: &str, value: i64) -> Self {
        Self {
            error_params: vec![value],
            ..Self::failure(symbol)
        }
    }
}
/// Early native Post gates have no command cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandGate {
    /// Tile outside the map or native void tile.
    Tile,
    /// Action forbidden at the configured pause level.
    Pause,
}
/// Observations of the actual phases entered by a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandReceipt {
    /// Native Post return value (final success), not merely enqueue acceptance.
    pub posted: bool,
    /// Early Post rejection, before a native cost exists.
    pub gate: Option<CommandGate>,
    /// Body test result before the outer funds check.
    pub test: Option<CommandCost>,
    /// Body execution result, before outer bookkeeping.
    pub exec: Option<CommandCost>,
    /// Final Execute result, including affordability errors.
    pub result: Option<CommandCost>,
    /// Actual native tuple values, absent for cost-only commands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<CommandReturnPhases>,
}
/// Native non-cost results, retaining successful tiles and invalid sentinels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandReturn {
    /// Native vehicle construction return tuple.
    Vehicle {
        /// Allocated ID, or native invalid ID 0xFFFFF.
        vehicle: u32,
        /// Default/refitted cargo capacity.
        capacity: u32,
        /// Mail capacity, zero for road vehicles.
        mail_capacity: u16,
        /// Capacity by native cargo slot.
        cargo_capacities: Box<CargoCapacities>,
    },
    /// Terraform/level-land native result tuple.
    Landscape {
        /// Additional cash required, distinct from the completed cost.
        additional_money: i64,
        /// Native returned tile, including zero and `INVALID_TILE`.
        tile: u32,
    },
}
/// Tuple values at exactly the command phases entered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandReturnPhases {
    /// Body test tuple before outer validation.
    pub test: Option<CommandReturn>,
    /// Body execution tuple before accounting.
    pub exec: Option<CommandReturn>,
    /// Final Execute tuple after outer validation/accounting.
    pub result: Option<CommandReturn>,
}
/// Rust scope or saved-state failure; never impersonates a native command error.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Runtime allocation or candidate-cache restoration failure.
    #[error(transparent)]
    Runtime(#[from] crate::runtime::RuntimeError),
    /// Vanilla specification or price restoration failure.
    #[error(transparent)]
    Content(#[from] crate::content::ContentError),
    /// A native gameplay context not implemented by this stage.
    #[error("unsupported command context: {0}")]
    Unsupported(&'static str),
    /// A saved field is absent or has an unexpected wire type.
    #[error(transparent)]
    Access(#[from] WorldAccessError),
    /// The resulting world failed save/structural validation.
    #[error(transparent)]
    World(#[from] WorldError),
    /// Arithmetic exceeded the supported native range.
    #[error("command arithmetic overflow in {0}")]
    Overflow(&'static str),
}
struct Plan {
    cost: CommandCost,
    edits: Vec<WorldEdit>,
    returns: Option<CommandReturn>,
}
impl Plan {
    const fn empty(cost: CommandCost) -> Self {
        Self {
            cost,
            edits: Vec::new(),
            returns: None,
        }
    }
}
fn body(world: &World, request: &CommandRequest) -> Result<Plan, CommandError> {
    match &request.command {
        Command::BuildRoadDepot { .. }
        | Command::BuildVehicle { .. }
        | Command::SellVehicle { .. } => Err(CommandError::Unsupported(
            "vehicle construction needs owned runtime",
        )),
        Command::ChangeServiceInterval {
            vehicle,
            interval,
            custom,
            percent,
        } => road_vehicle::service_interval(
            world,
            request.company,
            *vehicle,
            *interval,
            *custom,
            *percent,
        ),
        Command::LevelLand {
            tile,
            start_tile,
            diagonal,
            level_mode,
        } => level_land::estimate(
            world,
            request.company,
            level_land::Args {
                tile: *tile,
                start: *start_tile,
                diagonal: *diagonal,
                mode: *level_mode,
            },
        ),
        Command::TerraformLand {
            tile,
            slope,
            dir_up,
        } => terraform::plan(world, request.company, *tile, *slope, *dir_up),
        Command::IncreaseLoan { method, amount } => {
            finance::loan(world, request.company, (*method, *amount), true)
        }
        Command::DecreaseLoan { method, amount } => {
            finance::loan(world, request.company, (*method, *amount), false)
        }
        Command::RenameCompany { text } => naming::rename(world, request.company, text, false),
        Command::RenamePresident { text } => naming::rename(world, request.company, text, true),
        Command::Pause { mode, paused } => pause::pause(world, *mode, *paused),
        Command::BuildRoad { .. } => road::build(world, request.company, &request.command),
        Command::LandscapeClear { tile } => landscape::clear(world, request.company, *tile, false),
    }
}
