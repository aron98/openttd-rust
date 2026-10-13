use crate::{
    commands::{CommandError, landscape},
    runtime::{RoadStationGeometry, stations::saved},
};
use ottd_save::world::World;

pub(super) fn terminal_station(
    world: &World,
    station: u32,
    geometry: &RoadStationGeometry,
) -> Result<(), CommandError> {
    let row = saved::row(world, *b"STNN", station)?;
    for name in ["speclist", "roadstopspeclist", "roadstoptiledata"] {
        if !row.rows(name)?.1.is_empty() {
            return Err(CommandError::Unsupported(
                "joining custom station specifications",
            ));
        }
    }
    let area = geometry
        .station
        .ok_or(CommandError::Unsupported("joining deleted station"))?;
    for y in area.top..=area.bottom {
        for x in area.left..=area.right {
            let index = y
                .checked_mul(world.map().width())
                .and_then(|n| n.checked_add(x))
                .ok_or(CommandError::Overflow("station area tile"))?;
            let tile = landscape::tile_at(world, index)?;
            if tile.tile_type() >> 4 == 5 && u32::from(tile.m2()) == station && tile.m5() >= 4 {
                return Err(CommandError::Unsupported(
                    "joining drive-through road stops",
                ));
            }
        }
    }
    Ok(())
}
