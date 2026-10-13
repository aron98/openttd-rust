use super::{
    fields::ConsistField,
    input::Part,
    types::{ConsistError, ConsistRefusal},
};
use ottd_save::world::World;

#[derive(Clone, Copy)]
pub(super) struct TrackContext {
    owner: u8,
    tracks: u8,
    normal: bool,
    flat_position: bool,
}
impl TrackContext {
    pub(super) fn read(
        world: &World,
        tile_id: u32,
        position: [i32; 3],
    ) -> Result<Self, ConsistError> {
        let map = world.map();
        let index =
            usize::try_from(tile_id).map_err(|_| ConsistError::Field(ConsistField::Tile))?;
        let tile = map
            .tiles()
            .get(index)
            .ok_or(ConsistError::Field(ConsistField::Tile))?;
        let [x, y, z] = position;
        let owner = tile.m1() & 31;
        let flat = [1, map.width(), map.width().saturating_add(1)]
            .iter()
            .all(|offset| {
                tile_id
                    .checked_add(*offset)
                    .and_then(|id| usize::try_from(id).ok())
                    .and_then(|id| map.tiles().get(id))
                    .is_some_and(|other| other.height() == tile.height())
            });
        let positioned = match (u32::try_from(x), u32::try_from(y)) {
            (Ok(x), Ok(y)) => {
                Some(x / 16) == tile_id.checked_rem(map.width())
                    && Some(y / 16) == tile_id.checked_div(map.width())
                    && z == i32::from(tile.height()).wrapping_mul(8)
            }
            _ => false,
        };
        let company_exists = world
            .tables()
            .get(b"PLYR")
            .is_some_and(|table| table.records().contains_key(&u32::from(owner)));
        Ok(Self {
            owner,
            tracks: tile.m5(),
            normal: tile.tile_type() >> 4 == 1 && tile.m8().trailing_zeros() >= 6 && company_exists,
            flat_position: flat && positioned,
        })
    }
}

pub(super) fn validate(
    part: &Part,
    root: &Part,
    previous: Option<&Part>,
) -> Result<(), ConsistRefusal> {
    if part.owner != root.owner || part.context.owner != part.owner {
        return Err(ConsistRefusal::Owner);
    }
    if part.ground_flags & 3 != 0 {
        return Err(ConsistRefusal::SlopeFlags);
    }
    if part.flags & !8 != 0 {
        return Err(ConsistRefusal::Subtype);
    }
    if part.speed != 0 || part.status & 0x81 != 0 || (part.id == root.id && part.status & 2 == 0) {
        return Err(ConsistRefusal::Motion);
    }
    let direction_matches = match part.track {
        1 => matches!(part.direction, 1 | 5),
        2 => matches!(part.direction, 3 | 7),
        _ => false,
    };
    if !part.context.normal || part.context.tracks != part.track || !direction_matches {
        return Err(ConsistRefusal::Track);
    }
    if !part.context.flat_position || part.direction != root.direction || part.track != root.track {
        return Err(ConsistRefusal::Geometry);
    }
    if let Some(previous) = previous {
        let [px, py, pz] = previous.position;
        let [x, y, z] = part.position;
        let dx = i64::from(px).wrapping_sub(i64::from(x));
        let dy = i64::from(py).wrapping_sub(i64::from(y));
        let spacing = u32::from(8_u8.saturating_sub(previous.spec.shorten_factor.min(7)) / 2)
            .wrapping_add(u32::from(
                9_u8.saturating_sub(part.spec.shorten_factor.min(7)) / 2,
            ));
        let aligned = match part.direction {
            1 => dy == 0 && dx < 0,
            5 => dy == 0 && dx > 0,
            3 => dx == 0 && dy > 0,
            7 => dx == 0 && dy < 0,
            _ => false,
        };
        if !aligned || pz != z || dx.abs().max(dy.abs()) != i64::from(spacing) {
            return Err(ConsistRefusal::Geometry);
        }
    }
    Ok(())
}
