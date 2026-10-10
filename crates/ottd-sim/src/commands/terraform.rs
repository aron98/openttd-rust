mod candidate;
mod live;
mod surfaces;
pub(super) use live::body;

use super::{CommandCost, CommandError, CommandReturn, Plan};
use ottd_save::world::WorldEdit;

#[derive(Clone, Copy)]
pub(super) struct Args {
    pub(super) tile: u32,
    pub(super) mask: u8,
    pub(super) up: bool,
}

#[derive(Debug)]
enum Failure {
    Scope(CommandError),
    Native { cost: CommandCost, tile: u32 },
}
impl From<CommandError> for Failure {
    fn from(error: CommandError) -> Self {
        Self::Scope(error)
    }
}
fn native(symbol: &str, tile: u32) -> Failure {
    Failure::Native {
        cost: CommandCost::failure(symbol),
        tile,
    }
}
const fn outcome(cost: CommandCost, tile: u32, edits: Vec<WorldEdit>) -> Plan {
    Plan {
        cost,
        edits,
        returns: Some(CommandReturn::Landscape {
            additional_money: 0,
            tile,
        }),
    }
}
