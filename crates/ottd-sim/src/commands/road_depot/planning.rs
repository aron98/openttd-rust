use super::{Args, occupancy, record};
use crate::{
    commands::{CommandCost, CommandError, landscape, terrain_read::TerrainRead},
    content::{Price, VehicleSpec},
    runtime::DepotContext,
    world_access::{signed, unsigned},
};
use ottd_save::{
    TileRawParts,
    world::{World, WorldEdit},
};

pub(super) struct Build {
    pub cost: CommandCost,
    pub edits: Vec<WorldEdit>,
    pub create: bool,
}
impl Build {
    const fn rejected(cost: CommandCost) -> Self {
        Self {
            cost,
            edits: Vec::new(),
            create: false,
        }
    }
}
fn available(
    world: &World,
    company: u8,
    road_type: u8,
    context: &DepotContext<'_>,
) -> Result<bool, CommandError> {
    if road_type >= 63 {
        return Ok(false);
    }
    let date = signed(world, b"DATE", 0, "date")?;
    if road_type == 0 && date >= 0 {
        return Ok(true);
    }
    let climate = unsigned(world, b"PATS", 0, "game_creation.landscape")?;
    let mask = 1_u64
        .checked_shl(u32::try_from(climate).map_err(|_| CommandError::Overflow("climate"))?)
        .unwrap_or(0);
    for (index, engine) in context.content.engines().iter().enumerate() {
        let VehicleSpec::Road(spec) = engine.vehicle else {
            continue;
        };
        if spec.roadtype != road_type || u64::from(engine.info.climates) & mask == 0 {
            continue;
        }
        let id = u32::try_from(index).map_err(|_| CommandError::Overflow("engine ID"))?;
        if unsigned(world, b"ENGN", id, "company_avail")? & (1_u64 << company) != 0
            || date >= signed(world, b"ENGN", id, "intro_date")?.saturating_add(365)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn validate(
    world: &World,
    company: u8,
    args: Args,
    context: &DepotContext<'_>,
) -> Result<Build, CommandError> {
    if !available(world, company, args.road_type, context)? || args.direction >= 4 {
        return Ok(Build::rejected(CommandCost::failure("CMD_ERROR")));
    }
    let source = landscape::tile_at(world, args.tile)?;
    let (slope, _) = crate::terrain::tile_slope_z(world, args.tile)
        .map_err(|_| CommandError::Unsupported("depot slope geometry"))?;
    let bits = slope.raw();
    let mut cost = if bits == 0 {
        0
    } else {
        let entrance = match args.direction {
            0 => 12,
            1 => 6,
            2 => 3,
            3 => 9,
            _ => return Err(CommandError::Unsupported("depot direction")),
        };
        let fits = if bits & 16 != 0 {
            bits & entrance == entrance
        } else {
            bits & entrance != 0
        };
        if unsigned(world, b"PATS", 0, "construction.build_on_slopes")? == 0 || !fits {
            return Ok(Build::rejected(CommandCost::failure(
                "STR_ERROR_FLAT_LAND_REQUIRED",
            )));
        }
        context.content.price(Price::BuildFoundation)
    };
    let depot = source.tile_type() >> 4 == 2 && source.m5() >> 6 == 2;
    let tram =
        u8::try_from((source.m8() >> 6) & 63).map_err(|_| CommandError::Overflow("tram type"))?;
    let road_type = if tram == 63 { source.m4() & 63 } else { tram };
    let mut edits = Vec::new();
    let create = !(depot && road_type == args.road_type);
    if create {
        if depot {
            return clearing_error(
                CommandCost::failure("STR_ERROR_BUILDING_MUST_BE_DEMOLISHED"),
                cost,
            );
        }
        let cleared = landscape::clear_with_prices(
            TerrainRead::Committed(world),
            company,
            args.tile,
            true,
            context.content.prices(),
        )?;
        if !cleared.cost.success {
            return clearing_error(cleared.cost, cost);
        }
        cost = cost
            .checked_add(cleared.cost.cost)
            .ok_or(CommandError::Overflow("depot clearing cost"))?;
        if source.tile_type() & 0x0C != 0 {
            return Ok(Build::rejected(CommandCost::failure(
                "STR_ERROR_MUST_DEMOLISH_BRIDGE_FIRST",
            )));
        }
        if !context.pool.can_allocate(1) {
            return Ok(Build::rejected(CommandCost::failure("CMD_ERROR")));
        }
        edits = cleared.edits;
    } else {
        let owner = source.m1() & 31;
        if owner != company {
            return Ok(Build::rejected(ownership_error(world, args.tile, owner)?));
        }
        if source.m5() & 3 == args.direction {
            return Ok(Build::rejected(CommandCost::success(0, 255)));
        }
        if let Some(error) = occupancy::occupied(world, args.tile)? {
            return Ok(Build::rejected(error));
        }
        let mut tile = TileRawParts::from(source);
        tile.m5 = (tile.m5 & !3) | args.direction;
        edits.push(WorldEdit::Tile {
            index: args.tile,
            value: tile.into(),
        });
    }
    cost = cost
        .checked_add(context.content.price(Price::BuildDepotRoad))
        .ok_or(CommandError::Overflow("depot build cost"))?;
    Ok(Build {
        cost: CommandCost::success(cost, 0),
        edits,
        create,
    })
}
fn clearing_error(mut error: CommandCost, accumulated: i64) -> Result<Build, CommandError> {
    error.cost = accumulated
        .checked_add(error.cost)
        .ok_or(CommandError::Overflow("depot clearing cost"))?;
    error.expenses = 0;
    Ok(Build::rejected(error))
}

pub(super) fn ownership_error(
    world: &World,
    tile: u32,
    owner: u8,
) -> Result<CommandCost, CommandError> {
    let mut error = CommandCost::failure("STR_ERROR_OWNED_BY");
    error.error_params = match owner {
        0..=14
            if world
                .tables()
                .get(b"PLYR")
                .is_some_and(|table| table.records().contains_key(&u32::from(owner))) =>
        {
            vec![0x881D, i64::from(owner)]
        }
        15 => vec![
            0x8827,
            i64::from(
                record::nearest_town(world, tile)?
                    .ok_or(CommandError::Unsupported("town owner without town"))?,
            ),
        ],
        _ => {
            return Err(CommandError::Unsupported(
                "non-company depot ownership receipt",
            ));
        }
    };
    Ok(error)
}
