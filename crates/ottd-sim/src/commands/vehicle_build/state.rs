use super::{Args, CommandError, EngineSpec, RoadSpec};
use crate::{
    runtime::{RoadBuildState, SavedEngineView},
    world_access::{field, row_field, signed, unsigned},
};
use ottd_save::{WireValue, world::World};

pub(super) fn new(
    world: &World,
    company: u8,
    args: Args,
    engine: &EngineSpec,
    spec: RoadSpec,
    unit: u16,
    value: i64,
) -> Result<RoadBuildState, CommandError> {
    let width = std::num::NonZeroU32::new(world.map().width())
        .ok_or(CommandError::Overflow("map width"))?;
    let x = args.tile % width;
    let y = args.tile / width;
    let mut height = 0;
    for (x, y) in [
        (x, y),
        (x.saturating_add(1), y),
        (x, y.saturating_add(1)),
        (x.saturating_add(1), y.saturating_add(1)),
    ] {
        let tile = y
            .checked_mul(width.get())
            .and_then(|v| v.checked_add(x))
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| world.map().tiles().get(i))
            .ok_or(CommandError::Unsupported("depot map edge"))?;
        height = height.max(tile.height());
    }
    let tile = world
        .map()
        .tiles()
        .get(usize::try_from(args.tile).map_err(|_| CommandError::Overflow("tile"))?)
        .ok_or(CommandError::Unsupported("depot tile"))?;
    let (service_interval, service_percent) = service(world, company)?;
    let calendar_date = i32::try_from(signed(world, b"DATE", 0, "date")?)
        .map_err(|_| CommandError::Overflow("calendar date"))?;
    let build_year = ottd_core::CalendarDate::from_raw(calendar_date)
        .map_err(|_| CommandError::Unsupported("calendar date"))?
        .ymd()
        .0;
    let dynamic = SavedEngineView::new(world, args.engine)?;
    Ok(RoadBuildState {
        owner: company,
        unit,
        tile: args.tile,
        x: x.saturating_mul(16).saturating_add(8),
        y: y.saturating_mul(16).saturating_add(8),
        z: i32::from(height).saturating_mul(8),
        direction: (tile.m5() & 3).saturating_mul(2).saturating_add(1),
        engine: args.engine,
        image: spec.image_index,
        cargo: engine.info.cargo_type,
        capacity: u16::from(spec.capacity),
        reliability: dynamic.reliability()?,
        reliability_decay: dynamic.reliability_decay()?,
        max_age: engine
            .info
            .lifelength
            .wrapping_add(
                i32::try_from(unsigned(world, b"PATS", 0, "vehicle.extend_vehicle_life")?)
                    .map_err(|_| CommandError::Overflow("extended life"))?,
            )
            .wrapping_mul(366),
        economy_date: i32::try_from(signed(world, b"DATE", 0, "economy_date")?)
            .map_err(|_| CommandError::Overflow("economy date"))?,
        calendar_date,
        build_year,
        service_interval,
        service_percent,
        preview: dynamic.flags()? & 2 != 0,
        value,
        random_bits: 0,
    })
}
fn service(world: &World, company: u8) -> Result<(u16, bool), CommandError> {
    let table = world
        .tables()
        .get(b"PLYR")
        .ok_or(CommandError::Unsupported("company table"))?;
    let schema = table
        .schema()
        .fields()
        .iter()
        .find(|f| f.name() == "settings")
        .and_then(ottd_save::FieldSchema::child)
        .ok_or(CommandError::Unsupported("company settings"))?;
    let WireValue::Structs(rows) = field(world, b"PLYR", u32::from(company), "settings")? else {
        return Err(CommandError::Unsupported("company settings"));
    };
    let [row] = rows.as_slice() else {
        return Err(CommandError::Unsupported("company settings"));
    };
    let WireValue::Unsigned(interval) = row_field(schema, row, "settings.vehicle.servint_roadveh")?
    else {
        return Err(CommandError::Unsupported("company service interval"));
    };
    let WireValue::Signed(percent) = row_field(schema, row, "settings.vehicle.servint_ispercent")?
    else {
        return Err(CommandError::Unsupported("company service units"));
    };
    Ok((
        u16::try_from(*interval).map_err(|_| CommandError::Overflow("service interval"))?,
        *percent != 0,
    ))
}
