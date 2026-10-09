use super::{CommandCost, CommandError, Plan};
use crate::world_access::{field_edit, unsigned};
use ottd_save::{WireValue, world::World};
pub(super) fn pause(world: &World, mode: u8, paused: bool) -> Result<Plan, CommandError> {
    if !matches!(mode, 0 | 1 | 3 | 5 | 6) {
        return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
    }
    let old = unsigned(world, b"DATE", 0, "pause_mode")?;
    if mode == 0 && old & 8 != 0 {
        return Err(CommandError::Unsupported(
            "interactive unsafe unpause confirmation",
        ));
    }
    let bit = 1_u64 << mode;
    let mut new = if paused { old | bit } else { old & !bit };
    if !paused && new == 128 {
        new = 0;
    }
    Ok(Plan {
        cost: CommandCost::success(0, 255),
        edits: vec![field_edit(
            *b"DATE",
            0,
            "pause_mode",
            WireValue::Unsigned(new),
        )],
    })
}
