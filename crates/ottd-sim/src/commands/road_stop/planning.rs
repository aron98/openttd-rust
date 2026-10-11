use super::BuildRoadStop;
use crate::{
    commands::{
        CommandCost, CommandError, Plan, landscape, road_depot, terrain_read::TerrainRead,
        town_rating,
    },
    content::Price,
    runtime::{
        StationContext,
        stations::{self, saved},
    },
    world_access::unsigned,
};
use ottd_save::world::World;

pub(super) fn validate(
    world: &World,
    company: u8,
    tile: u32,
    args: BuildRoadStop,
    context: &StationContext<'_>,
) -> Result<Plan, CommandError> {
    if let Some(rejected) = parameters(world, company, tile, args, context)? {
        return Ok(rejected);
    }
    let joining = args.station_to_join < 65533;
    let spread = unsigned(world, b"PATS", 0, "station.station_spread")?;
    if args.width != 1
        || args.length != 1
        || args.drive_through
        || args.road_type != 0
        || !args.adjacent
        || args.station_to_join == 65535
    {
        return Err(CommandError::Unsupported(
            "terminal explicit road station construction",
        ));
    }
    if tile.checked_rem(world.map().width()) == Some(0) {
        return Err(CommandError::Unsupported("station rectangle sentinel edge"));
    }
    if let Some(cost) = road_depot::occupancy::occupied(world, tile)? {
        return Ok(Plan::empty(cost));
    }
    let (slope, _) = crate::terrain::tile_slope_z(world, tile)
        .map_err(|_| CommandError::Unsupported("road stop slope"))?;
    if slope.raw() != 0 {
        return Err(CommandError::Unsupported("sloped road stop construction"));
    }
    if landscape::tile_at(world, tile)?.tile_type() & 0x0c != 0 {
        return Err(CommandError::Unsupported("road stop under bridge"));
    }
    let mut plan = landscape::clear_with_prices(
        TerrainRead::Committed(world),
        company,
        tile,
        true,
        context.content.prices(),
    )?;
    if !plan.cost.success {
        return Ok(plan);
    }
    if !context.stations.stops.can_allocate(1) {
        return Ok(reject(if args.stop_type == 0 {
            "STR_ERROR_TOO_MANY_BUS_STOPS"
        } else {
            "STR_ERROR_TOO_MANY_TRUCK_STOPS"
        }));
    }
    if joining {
        let normal =
            saved::row(world, *b"STNN", u32::from(args.station_to_join))?.child("normal")?;
        if normal.child("base")?.number("owner")? != u64::from(company) {
            return Ok(reject("CMD_ERROR"));
        }
        let Some(current) = context
            .stations
            .geometry
            .get(&u32::from(args.station_to_join))
        else {
            return Err(CommandError::Unsupported(
                "joining non-road station facilities",
            ));
        };
        let mut geometry = current.clone();
        geometry.add(
            tile,
            args.stop_type == 0,
            (world.map().width(), world.map().height()),
            stations::catchment_radius(world)?,
        )?;
        if geometry.station.is_some_and(|area| {
            let (w, h) = area.dimensions();
            u64::from(w) > spread || u64::from(h) > spread
        }) {
            return Ok(reject("STR_ERROR_STATION_TOO_SPREAD_OUT"));
        }
        super::acceptance::terminal_station(world, u32::from(args.station_to_join), current)?;
    } else if !context.stations.stations.can_allocate(1) {
        return Ok(reject("STR_ERROR_TOO_MANY_STATIONS_LOADING"));
    }
    let build = context.content.price(if args.stop_type == 0 {
        Price::BuildStationBus
    } else {
        Price::BuildStationTruck
    });
    plan.cost.cost = plan
        .cost
        .cost
        .checked_add(build)
        .and_then(|n| n.checked_add(context.content.price(Price::BuildRoad).checked_mul(2)?))
        .ok_or(CommandError::Overflow("road stop construction price"))?;
    Ok(plan)
}
fn reject(error: &'static str) -> Plan {
    Plan::empty(CommandCost::failure(error))
}

fn parameters(
    world: &World,
    company: u8,
    tile: u32,
    args: BuildRoadStop,
    context: &StationContext<'_>,
) -> Result<Option<Plan>, CommandError> {
    if !road_depot::planning::available(world, company, args.road_type, context.content)?
        || args.direction >= 4
        || args.stop_type >= 2
        || args.spec_class != 0
        || args.spec_index != 0
    {
        return Ok(Some(reject("CMD_ERROR")));
    }
    let spread = unsigned(world, b"PATS", 0, "station.station_spread")?;
    if u64::from(args.width) > spread || u64::from(args.length) > spread {
        return Ok(Some(reject("STR_ERROR_STATION_TOO_SPREAD_OUT")));
    }
    if args.width == 0 || args.length == 0 {
        return Ok(Some(reject("CMD_ERROR")));
    }
    let width = std::num::NonZeroU32::new(world.map().width())
        .ok_or(CommandError::Overflow("map width"))?;
    if (tile % width).saturating_add(u32::from(args.width)) > width.get()
        || (tile / width).saturating_add(u32::from(args.length)) > world.map().height()
    {
        return Ok(Some(reject("CMD_ERROR")));
    }
    if landscape::tile_at(world, tile)?.tile_type() >> 4 == 7 {
        return Ok(Some(reject("CMD_ERROR")));
    }
    let joining = args.station_to_join < 65533;
    if joining
        && (unsigned(world, b"PATS", 0, "station.distant_join_stations")? == 0
            || !world
                .tables()
                .get(b"STNN")
                .is_some_and(|t| t.records().contains_key(&u32::from(args.station_to_join))))
    {
        return Ok(Some(reject("CMD_ERROR")));
    }
    if joining
        && saved::row(world, *b"STNN", u32::from(args.station_to_join))?.number("facilities")? & 128
            != 0
    {
        return Ok(Some(reject("CMD_ERROR")));
    }
    if args.station_to_join == 65534 || (args.road_type == 1 && !args.drive_through) {
        return Ok(Some(reject("CMD_ERROR")));
    }
    if unsigned(world, b"PATS", 0, "difficulty.town_council_tolerance")? != 3 {
        if let Some(town) = town_rating::nearest(TerrainRead::Committed(world), tile)? {
            if town_rating::saved_rating(TerrainRead::Committed(world), town, company)? <= -200 {
                let mut cost =
                    CommandCost::failure("STR_ERROR_LOCAL_AUTHORITY_REFUSES_TO_ALLOW_THIS");
                cost.error_params.push(i64::from(town));
                return Ok(Some(Plan::empty(cost)));
            }
        }
    }
    Ok(None)
}
