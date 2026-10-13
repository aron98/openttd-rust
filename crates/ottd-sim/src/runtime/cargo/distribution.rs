//! Source station eligibility and native fractional company/rating distribution.
use super::{RuntimeError, graph, read::Read, write};
use crate::{
    WorldTickError, WorldTickState as State,
    cargo::{CargoSource, Production},
    runtime::{stations::StationRuntime, transport::TransportRuntime},
    unsupported_tick as unsupported,
};
use std::collections::BTreeMap;

struct Recipient {
    station: u32,
    owner: u8,
    rating: u32,
    amount: u32,
}
impl TransportRuntime {
    pub(in crate::runtime) fn town_production(
        &mut self,
        state: &mut State<'_>,
        production: Production,
        stations: &StationRuntime,
    ) -> Result<u32, WorldTickError> {
        let CargoSource::Town(town) = production.source else {
            return Err(unsupported("cargo_distribution", "town producer source"));
        };
        let tile = state.view().tile(production.area.tile)?;
        if production.area.width != 1
            || production.area.height != 1
            || tile.tile_type() >> 4 != 3
            || tile.m2() != town
        {
            return Err(unsupported("cargo_distribution", "town source area"));
        }
        let nearby = self
            .cargo
            .town_near
            .as_ref()
            .and_then(|towns| towns.get(&u32::from(town)))
            .ok_or_else(|| unsupported("cargo_distribution", "town recipient cache unavailable"))?;
        let mut found = Vec::new();
        for id in nearby {
            let geometry = stations
                .geometry
                .get(id)
                .ok_or(RuntimeError::Invalid("cargo station geometry"))?;
            if geometry.catchment.contains(&production.area.tile) {
                found.push(*id);
            }
        }
        self.distribute(state, production, &found, u8::MAX)
    }
    pub(in crate::runtime) fn distribute(
        &mut self,
        state: &mut State<'_>,
        production: Production,
        stations: &[u32],
        exclusive: u8,
    ) -> Result<u32, WorldTickError> {
        if production.amount == 0 || stations.is_empty() {
            return Ok(0);
        }
        let mut recipients = Vec::new();
        for station in stations {
            if let Some(recipient) = eligible(state, *station, production.cargo, exclusive)? {
                recipients.push(recipient);
            }
        }
        if recipients.is_empty() {
            return Ok(0);
        }
        if let [recipient] = recipients.as_mut_slice() {
            recipient.amount = production
                .amount
                .wrapping_mul(recipient.rating.wrapping_add(1));
        } else {
            let mut companies = BTreeMap::<u8, (u32, u32)>::new();
            for r in &recipients {
                let (best, sum) = companies.entry(r.owner).or_default();
                *best = (*best).max(r.rating);
                *sum = sum.wrapping_add(r.rating);
            }
            let best_sum = companies
                .values()
                .fold(0_u32, |sum, (best, _)| sum.wrapping_add(*best));
            let best = recipients
                .iter()
                .map(|r| r.rating)
                .max()
                .ok_or(RuntimeError::Invalid("cargo recipient ratings"))?;
            let amount = production.amount.wrapping_mul(best.wrapping_add(1));
            let mut moving = 0_u32;
            for r in &mut recipients {
                let (best, sum) = companies
                    .get(&r.owner)
                    .copied()
                    .ok_or(RuntimeError::Invalid("cargo owner rating"))?;
                r.amount = amount
                    .wrapping_mul(best)
                    .wrapping_mul(r.rating)
                    .checked_div(best_sum)
                    .and_then(|value| value.checked_div(sum))
                    .ok_or(RuntimeError::Invalid("recipient rating divisors"))?;
                moving = moving.wrapping_add(r.amount);
            }
            if amount > moving {
                recipients.sort_by_key(|recipient| std::cmp::Reverse(recipient.rating));
                let remainder = usize::try_from(amount.wrapping_sub(moving))
                    .map_err(|_| unsupported("cargo_distribution", "rounding remainder"))?;
                if remainder > recipients.len() {
                    return Err(unsupported(
                        "cargo_distribution",
                        "native rounding invariant",
                    ));
                }
                for r in recipients.iter_mut().take(remainder) {
                    r.amount = r.amount.wrapping_add(1);
                }
            }
        }
        let mut moved = 0_u32;
        for r in recipients {
            moved = moved.wrapping_add(self.waiting(state, r.station, production, r.amount)?);
        }
        Ok(moved)
    }
    fn waiting(
        &mut self,
        state: &mut State<'_>,
        station: u32,
        production: Production,
        amount: u32,
    ) -> Result<u32, WorldTickError> {
        if !self.cargo.pool.can_allocate(1) {
            return Ok(0);
        }
        let fraction =
            u32::try_from(write::goods(state, station, production.cargo)?.number("amount_fract")?)
                .map_err(|_| unsupported("cargo_distribution", "fraction"))?;
        let amount = amount.wrapping_add(fraction);
        write::goods_field(
            state,
            station,
            production.cargo,
            "amount_fract",
            u64::from(amount & 255),
        )?;
        let amount = amount >> 8;
        if amount == 0 {
            return Ok(0);
        }
        self.cargo.append_production(
            state,
            station,
            production.cargo,
            u16::try_from(amount).map_err(|_| unsupported("cargo_distribution", "packet size"))?,
            production.source,
        )?;
        graph::supply(state, self, station, production.cargo, amount)?;
        let status = write::goods(state, station, production.cargo)?.number("status")?;
        write::goods_field(state, station, production.cargo, "status", status | 2)?;
        Ok(amount)
    }
}
fn eligible(
    state: &State<'_>,
    station: u32,
    cargo: u8,
    exclusive: u8,
) -> Result<Option<Recipient>, WorldTickError> {
    let row = Read::Candidate(state.view()).row(*b"STNN", station)?;
    let base = row.child("normal")?.child("base")?;
    let owner = u8::try_from(base.number("owner")?)
        .map_err(|_| unsupported("cargo_distribution", "station owner"))?;
    if exclusive != u8::MAX && exclusive != owner {
        return Ok(None);
    }
    let town = u32::try_from(
        base.number("town")?
            .checked_sub(1)
            .ok_or(RuntimeError::Invalid("station town pointer"))?,
    )
    .map_err(|_| unsupported("cargo_distribution", "station town"))?;
    if owner != 0x10
        && state.number(b"CITY", town, "exclusive_counter")? > 0
        && state.number(b"CITY", town, "exclusivity")? != i64::from(owner)
    {
        return Ok(None);
    }
    let goods = write::goods(state, station, cargo)?;
    let rating = u32::try_from(goods.number("rating")?)
        .map_err(|_| unsupported("cargo_distribution", "rating"))?;
    if rating == 0
        || (state.number(b"PATS", 0, "order.selectgoods")? != 0 && goods.number("last_speed")? == 0)
    {
        return Ok(None);
    }
    let facilities = row.number("facilities")?;
    if (cargo == 0 && facilities == 2) || (cargo != 0 && facilities == 4) {
        return Ok(None);
    }
    Ok(Some(Recipient {
        station,
        owner,
        rating,
        amount: 0,
    }))
}
