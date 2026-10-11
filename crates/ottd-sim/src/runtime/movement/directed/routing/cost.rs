//! `CYapfCostRoadT` segment boundaries and destination octile estimate.
use super::{
    Node, WorldTickError,
    network::{Network, Position, TileKind},
    unsupported,
};
pub(super) struct Destination {
    pub station: u16,
    pub kind: u8,
    pub tile: u32,
}
pub(super) struct Costs<'a, 'w> {
    pub network: Network<'a, 'w>,
    pub destination: Destination,
    pub curve: i64,
    pub occupied: i64,
}
impl Costs<'_, '_> {
    pub(super) fn target(&self, position: Position) -> Result<bool, WorldTickError> {
        Ok(matches!(self.network.tile(position.tile)?.kind,
            TileKind::Terminal { station, kind, .. } if station == self.destination.station && kind == self.destination.kind))
    }
    fn tile_cost(&self, position: Position) -> Result<i64, WorldTickError> {
        if !matches!(position.track, 0 | 1 | 8 | 9) {
            return Ok(71_i64.wrapping_add(self.curve));
        }
        let occupancy = match self.network.tile(position.tile)?.kind {
            TileKind::Terminal { status, .. } => {
                i64::from(2_u32.wrapping_sub((status & 3).count_ones()))
            }
            TileKind::Road | TileKind::Depot(_) | TileKind::Empty => 0,
        };
        Ok(100_i64.wrapping_add(self.occupied.wrapping_mul(occupancy) / 2))
    }
    pub(super) fn evaluate(&self, node: &mut Node) -> Result<bool, WorldTickError> {
        let mut tile_count = 0_u32;
        loop {
            node.cost = node.cost.wrapping_add(self.tile_cost(node.end)?);
            if self.target(node.end)? {
                break;
            }
            if matches!(self.network.tile(node.end.tile)?.kind, TileKind::Depot(direction) if direction ^ 2 == node.end.exit()?)
            {
                break;
            }
            let Some((tile, bits)) = self.network.follow(node.end)? else {
                break;
            };
            if bits.count_ones() != 1 {
                break;
            }
            let track = u8::try_from(bits.trailing_zeros())
                .map_err(|_| unsupported("road_yapf", "track bits"))?;
            let next = Position { tile, track };
            if next == node.start {
                return Ok(false);
            }
            node.end = next;
            tile_count = tile_count.wrapping_add(1);
            if tile_count > 4096 {
                break;
            }
        }
        node.estimate = node.cost;
        if !self.target(node.end)? {
            let width = i64::from(self.network.state.width);
            let x = i64::from(node.end.tile) & width.wrapping_sub(1);
            let y = i64::from(node.end.tile) >> width.trailing_zeros();
            let (sx, sy) = match node.end.exit()? {
                0 => (-1, 0),
                1 => (0, 1),
                2 => (1, 0),
                3 => (0, -1),
                _ => return Err(unsupported("road_yapf", "estimate direction")),
            };
            let dx = (x.wrapping_mul(2).wrapping_add(sx).wrapping_sub(
                (i64::from(self.destination.tile) & width.wrapping_sub(1)).wrapping_mul(2),
            ))
            .abs();
            let dy = (y.wrapping_mul(2).wrapping_add(sy).wrapping_sub(
                (i64::from(self.destination.tile) >> width.trailing_zeros()).wrapping_mul(2),
            ))
            .abs();
            let estimate = dx
                .min(dy)
                .wrapping_mul(71)
                .wrapping_add(dx.wrapping_sub(dy).abs().wrapping_sub(1).wrapping_mul(50));
            node.estimate = node.cost.wrapping_add(estimate);
        }
        Ok(true)
    }
}
