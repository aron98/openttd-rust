mod phases;
#[cfg(test)]
mod tests;
mod tiles;
use super::{
    CommandCost, CommandError, CommandReturn, terraform,
    terrain_context::{TerrainContext, TerrainFlags},
    terrain_read::TerrainRead,
    terrain_state::TerrainState,
};
pub(super) use phases::run;
#[cfg(test)]
pub(super) use phases::run_context;
use tiles::Tiles;

#[derive(Clone, Copy)]
pub(super) struct Args {
    pub tile: u32,
    pub start: u32,
    pub diagonal: bool,
    pub mode: u8,
}
enum LevelMode {
    Level,
    Raise,
    Lower,
}
impl TryFrom<u8> for LevelMode {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Level),
            1 => Ok(Self::Lower),
            2 => Ok(Self::Raise),
            _ => Err(()),
        }
    }
}
struct Outcome {
    cost: CommandCost,
    additional_money: i64,
    tile: u32,
}
impl Outcome {
    const fn new(cost: CommandCost, tile: u32) -> Self {
        Self {
            cost,
            additional_money: 0,
            tile,
        }
    }
    const fn result(&self) -> CommandReturn {
        CommandReturn::Landscape {
            additional_money: self.additional_money,
            tile: self.tile,
        }
    }
    fn failure(symbol: &str) -> Self {
        Self::new(CommandCost::failure(symbol), u32::MAX)
    }
}
fn land(
    state: &mut TerrainState<'_, '_>,
    context: &mut TerrainContext,
    args: Args,
    flags: TerrainFlags,
) -> Result<Outcome, CommandError> {
    let view = state.read();
    let company = context.company;
    let size = view.size();
    if args.start >= size.count()? {
        return Ok(Outcome::failure("CMD_ERROR"));
    }
    let Ok(mode) = LevelMode::try_from(args.mode) else {
        return Ok(Outcome::failure("CMD_ERROR"));
    };
    let old = u64::from(view.tile(args.start)?.height());
    let target = match mode {
        LevelMode::Level => old,
        LevelMode::Raise => old.saturating_add(1),
        LevelMode::Lower => old.wrapping_sub(1),
    };
    if target > view.unsigned(*b"PATS", 0, "construction.map_height_limit")? {
        return Ok(Outcome::failure(if old == 0 {
            "STR_ERROR_ALREADY_AT_SEA_LEVEL"
        } else {
            "STR_ERROR_TOO_HIGH"
        }));
    }
    let mut money = available_money(view, company)?;
    let mut limit =
        u32::try_from(view.unsigned(*b"PLYR", u32::from(company), "terraform_limit")?)
            .map_err(|_| CommandError::Overflow("terraform limit"))?
            >> 16;
    if limit == 0 {
        return Ok(Outcome::failure("STR_ERROR_TERRAFORM_LIMIT_REACHED"));
    }
    let mut cost = 0_i64;
    let mut last_error = CommandCost::failure(match mode {
        LevelMode::Level => "STR_ERROR_ALREADY_LEVELLED",
        LevelMode::Raise | LevelMode::Lower => "CMD_ERROR",
    });
    let mut had_success = false;
    let mut error_tile = u32::MAX;
    for tile in Tiles::new(size, args.tile, args.start, args.diagonal)? {
        let mut current = u64::from(state.read().tile(tile)?.height());
        while current != target {
            let terraform_args = terraform::Args {
                tile,
                mask: 8,
                up: current <= target,
            };
            let plan = context.test(|context| {
                terraform::body(state, context, terraform_args, TerrainFlags(flags.0 & !1))
            })?;
            let Some(CommandReturn::Landscape { tile: returned, .. }) = plan.returns else {
                return Err(CommandError::Unsupported("terraform tuple missing"));
            };
            error_tile = returned;
            if !plan.cost.success {
                if plan.cost.error.as_deref() == Some("STR_ERROR_TERRAFORM_LIMIT_REACHED") {
                    limit = 0;
                }
                last_error = plan.cost;
                break;
            }
            if flags.executing() {
                money = money.saturating_sub(plan.cost.cost);
                if money < 0 {
                    return Ok(Outcome {
                        cost: CommandCost::success(cost, 0),
                        additional_money: plan.cost.cost,
                        tile: error_tile,
                    });
                }
                terraform::body(state, context, terraform_args, flags)?;
            } else {
                limit = limit.saturating_sub(1);
                if limit == 0 {
                    had_success = true;
                    break;
                }
            }
            cost = cost.saturating_add(plan.cost.cost);
            current = if current > target {
                current.saturating_sub(1)
            } else {
                current.saturating_add(1)
            };
            had_success = true;
        }
        if limit == 0 {
            break;
        }
    }
    let result = if had_success {
        CommandCost::success(cost, 0)
    } else {
        last_error
    };
    let tile = if result.success {
        args.tile
    } else {
        error_tile
    };
    Ok(Outcome::new(result, tile))
}

fn available_money(view: TerrainRead<'_>, company: u8) -> Result<i64, CommandError> {
    if view.unsigned(*b"PATS", 0, "difficulty.infinite_money")? != 0 {
        Ok(i64::MAX)
    } else {
        view.signed(*b"PLYR", u32::from(company), "money")
    }
}
