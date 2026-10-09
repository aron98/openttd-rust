mod phases;
mod tiles;
use super::{
    CommandCost, CommandError, CommandReturn, Plan, terraform,
    terrain_read::{MapSize, TerrainRead},
};
use crate::content::Prices;
use ottd_save::world::WorldTransaction;
pub(super) use phases::{estimate, run};
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
enum Phase<'a, 'world> {
    Estimate(TerrainRead<'a>),
    Execute {
        transaction: &'a mut WorldTransaction<'world>,
        size: MapSize,
    },
}
impl Phase<'_, '_> {
    const fn read(&self) -> TerrainRead<'_> {
        match self {
            Self::Estimate(view) => *view,
            Self::Execute { transaction, size } => TerrainRead::Candidate {
                view: transaction.view(),
                size: *size,
            },
        }
    }
}
struct Outcome {
    cost: CommandCost,
    additional_money: i64,
    tile: u32,
}
impl Outcome {
    const fn result(&self) -> CommandReturn {
        CommandReturn::Landscape {
            additional_money: self.additional_money,
            tile: self.tile,
        }
    }
    fn failure(symbol: &str) -> Self {
        Self {
            cost: CommandCost::failure(symbol),
            additional_money: 0,
            tile: u32::MAX,
        }
    }
}
fn land(
    phase: &mut Phase<'_, '_>,
    company: u8,
    args: Args,
    prices: &Prices,
) -> Result<Outcome, CommandError> {
    let view = phase.read();
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
    let mut limit = (view.unsigned(*b"PLYR", u32::from(company), "terraform_limit")? >> 16) & 65535;
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
        let mut current = u64::from(phase.read().tile(tile)?.height());
        while current != target {
            let plan = terraform::step(phase.read(), company, tile, 8, current <= target, prices)?;
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
            match phase {
                Phase::Execute { transaction, .. } => {
                    money = money.saturating_sub(plan.cost.cost);
                    if money < 0 {
                        return Ok(Outcome {
                            cost: CommandCost::success(cost, 0),
                            additional_money: plan.cost.cost,
                            tile: error_tile,
                        });
                    }
                    for edit in plan.edits {
                        transaction.apply(edit)?;
                    }
                }
                Phase::Estimate(_) => {
                    limit = limit.saturating_sub(1);
                    if limit == 0 {
                        had_success = true;
                        break;
                    }
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
    Ok(Outcome {
        cost: result,
        additional_money: 0,
        tile,
    })
}

fn available_money(view: TerrainRead<'_>, company: u8) -> Result<i64, CommandError> {
    if view.unsigned(*b"PATS", 0, "difficulty.infinite_money")? != 0 {
        Ok(i64::MAX)
    } else {
        view.signed(*b"PLYR", u32::from(company), "money")
    }
}
