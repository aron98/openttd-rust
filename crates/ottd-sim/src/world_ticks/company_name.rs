//! Native named-town/president fallback at the existing per-company tick position.
use super::{WorldTickError, unsupported};
use crate::{
    WorldTickState as State,
    world_access::{field, row_field, unsigned},
};
use ottd_save::{
    TileState, WireValue,
    world::{CandidateView, World},
};

#[derive(Clone, Copy)]
enum Read<'a> {
    World(&'a World),
    Candidate(CandidateView<'a>, u32),
}
impl Read<'_> {
    fn value(self, chunk: [u8; 4], id: u32, name: &str) -> Result<WireValue, WorldTickError> {
        Ok(match self {
            Self::World(world) => field(world, &chunk, id, name)?.clone(),
            Self::Candidate(view, _) => {
                let table = view
                    .table(chunk)
                    .ok_or_else(|| unsupported("company_name", "table"))?;
                let record = table
                    .record(id)
                    .ok_or_else(|| unsupported("company_name", "row"))?;
                row_field(table.schema(), record, name)?.clone()
            }
        })
    }
    fn number(self, chunk: [u8; 4], id: u32, name: &str) -> Result<u64, WorldTickError> {
        if let Self::World(world) = self {
            return Ok(unsigned(world, &chunk, id, name)?);
        }
        match self.value(chunk, id, name)? {
            WireValue::Unsigned(value) => Ok(value),
            _ => Err(unsupported("company_name", "unsigned field")),
        }
    }
    fn tile(self, index: u32) -> Result<TileState, WorldTickError> {
        match self {
            Self::World(world) => world
                .map()
                .tiles()
                .get(usize::try_from(index).map_err(|_| unsupported("company_name", "tile"))?)
                .cloned()
                .ok_or_else(|| unsupported("company_name", "tile")),
            Self::Candidate(view, _) => Ok(view.tile(index)?),
        }
    }
    fn towns(self) -> Result<Vec<u32>, WorldTickError> {
        match self {
            Self::World(world) => Ok(world
                .tables()
                .get(b"CITY")
                .ok_or_else(|| unsupported("company_name", "town pool"))?
                .records()
                .keys()
                .copied()
                .collect()),
            Self::Candidate(view, _) => Ok(view
                .table(*b"CITY")
                .ok_or_else(|| unsupported("company_name", "town pool"))?
                .records()
                .map(|(id, _)| id)
                .collect()),
        }
    }
    const fn width(self) -> u32 {
        match self {
            Self::World(world) => world.map().width(),
            Self::Candidate(_, width) => width,
        }
    }
}
fn closest(view: Read<'_>, index: u32) -> Result<u32, WorldTickError> {
    let tile = view.tile(index)?;
    if tile.tile_type() >> 4 == 3 {
        return Ok(u32::from(tile.m2()));
    }
    if tile.tile_type() >> 4 == 2 && tile.m5() >> 6 != 2 {
        let owner = if tile.m5() >> 6 == 0 {
            tile.m1() & 31
        } else {
            tile.m7() & 31
        };
        if owner != 15 && tile.m2() == u16::MAX {
            return Err(unsupported("company_name", "road has no town"));
        }
        return Ok(u32::from(tile.m2()));
    }
    let width = std::num::NonZeroU32::new(view.width())
        .ok_or_else(|| unsupported("company_name", "map width"))?;
    let mut nearest = None;
    for id in view.towns()? {
        let xy = u32::try_from(view.number(*b"CITY", id, "xy")?)
            .map_err(|_| unsupported("company_name", "town tile"))?;
        let distance = (index % width)
            .abs_diff(xy % width)
            .wrapping_add((index / width).abs_diff(xy / width));
        let key = (distance, id);
        if nearest.is_none_or(|old| key < old) {
            nearest = Some(key);
        }
    }
    nearest
        .map(|(_, id)| id)
        .ok_or_else(|| unsupported("company_name", "no closest town"))
}
fn plan(view: Read<'_>, id: u32) -> Result<Option<u64>, WorldTickError> {
    if view.number(*b"PLYR", id, "name_1")? != 0x6001
        || view.number(*b"PLYR", id, "last_build_coordinate")? == 0
    {
        return Ok(None);
    }
    let tile = u32::try_from(view.number(*b"PLYR", id, "last_build_coordinate")?)
        .map_err(|_| unsupported("company_name", "build tile"))?;
    let town = closest(view, tile)?;
    let WireValue::Bytes(name) = view.value(*b"CITY", town, "name")? else {
        return Err(unsupported("company_name", "town name"));
    };
    let name_type = view.number(*b"CITY", town, "townnametype")?;
    if name.is_empty() && (0x20c0..0x20d5).contains(&name_type) {
        return Err(unsupported("company_name", "built-in town-name generation"));
    }
    if view.number(*b"PLYR", id, "president_name_1")? != 0x70e7 {
        return Err(unsupported(
            "company_name",
            "random president-name generation",
        ));
    }
    Ok(Some(view.number(*b"PLYR", id, "president_name_2")?))
}
pub(super) fn validate(world: &World, id: u32) -> Result<(), WorldTickError> {
    plan(Read::World(world), id).map(|_| ())
}
pub(super) fn tick(state: &mut State<'_>) -> Result<(), WorldTickError> {
    let id = u32::try_from(state.number(b"DATE", 0, "company_tick_counter")?)
        .map_err(|_| unsupported("company_name", "company index"))?;
    if state.table(b"PLYR")?.record(id).is_none() {
        return Ok(());
    }
    if let Some(seed) = plan(Read::Candidate(state.view(), state.width), id)? {
        state.set(b"PLYR", id, "name_1", WireValue::Unsigned(0x70e6))?;
        state.set(b"PLYR", id, "name_2", WireValue::Unsigned(seed))?;
    }
    Ok(())
}
