//! Station rating ticks preserve native cargo iteration and conditional RNG draws.
use super::{CargoRuntime, RuntimeError, read::Read, truncate::random, write};
use crate::{
    WorldTickError, WorldTickState as State, content::ContentCatalog,
    unsupported_tick as unsupported,
};
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};

fn normal(
    state: &mut State<'_>,
    station: u32,
    name: &str,
    value: u64,
) -> Result<(), WorldTickError> {
    state.apply(WorldEdit::Field {
        chunk: *b"STNN",
        record: station,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value: WireValue::Unsigned(value),
    })
}
impl CargoRuntime {
    pub(in crate::runtime) fn rate_station(
        &mut self,
        state: &mut State<'_>,
        station: u32,
        content: &ContentCatalog,
    ) -> Result<(), WorldTickError> {
        increment_timers(state, station)?;
        let mut waiting_changed = false;
        for (id, spec) in content
            .cargo()
            .iter()
            .enumerate()
            .filter(|(_, spec)| spec.bitnum != 255)
        {
            if spec.callback_mask != 0 {
                return Err(unsupported("station_rating", "cargo callback"));
            }
            let cargo = u8::try_from(id).map_err(|_| unsupported("station_rating", "cargo id"))?;
            let goods = write::goods(state, station, cargo)?;
            let status = goods.number("status")?;
            let old = goods.number("rating")?;
            if status & 2 == 0 {
                if old < 175 {
                    write::goods_field(state, station, cargo, "rating", old.wrapping_add(1))?;
                }
                continue;
            }
            let pickup = goods
                .number("time_since_pickup")?
                .saturating_add(1)
                .min(255);
            write::goods_field(state, station, cargo, "time_since_pickup", pickup)?;
            if pickup == 255 && state.number(b"PATS", 0, "order.selectgoods")? != 0 {
                write::goods_field(state, station, cargo, "status", status & !2)?;
                write::goods_field(state, station, cargo, "last_speed", 0)?;
                self.truncate(state, station, cargo, u32::MAX)?;
                waiting_changed = true;
                continue;
            }
            let list = self
                .stations
                .get(&station)
                .and_then(|goods| goods.get(&u32::from(cargo)));
            let mut waiting = list.map_or(0, |list| list.count);
            let destinations = u32::try_from(list.map_or(0, |list| list.packets.len()))
                .map_err(|_| unsupported("station_rating", "destinations"))?;
            let average = waiting
                .checked_div(destinations.wrapping_add(1))
                .ok_or(RuntimeError::Invalid("rating destination divisor"))?;
            let rating = target(state, station, cargo, pickup)?;
            let old =
                i64::try_from(old).map_err(|_| unsupported("station_rating", "old rating"))?;
            let rating = u32::try_from(
                (old.wrapping_add(rating.wrapping_sub(old).clamp(-2, 2))).clamp(0, 255),
            )
            .map_err(|_| unsupported("station_rating", "rating"))?;
            write::goods_field(state, station, cargo, "rating", u64::from(rating))?;
            if rating <= 64 && average >= 100 {
                let mut decrease = random(state)? & 31;
                if average < 200 {
                    decrease &= 7;
                }
                waiting = waiting.wrapping_sub(decrease.wrapping_add(1).wrapping_mul(destinations));
                waiting_changed = true;
            }
            if rating <= 127 && waiting != 0 {
                let value = random(state)?;
                if rating <= value & 127 {
                    waiting = waiting.saturating_sub(
                        ((value >> 8) & 3)
                            .wrapping_add(1)
                            .wrapping_mul(destinations),
                    );
                    waiting_changed = true;
                }
            }
            if waiting > 4096 {
                waiting = waiting
                    .wrapping_sub(waiting.wrapping_sub(4096) / 64)
                    .min(32768);
                waiting_changed = true;
            }
            let count = self
                .stations
                .get(&station)
                .and_then(|goods| goods.get(&u32::from(cargo)))
                .map_or(0, |list| list.count);
            if waiting_changed && waiting < count {
                write::goods_field(state, station, cargo, "max_waiting_cargo", 0)?;
                self.truncate(state, station, cargo, count.wrapping_sub(waiting))?;
            } else {
                write::goods_field(
                    state,
                    station,
                    cargo,
                    "max_waiting_cargo",
                    u64::from(average),
                )?;
            }
        }
        Ok(())
    }
}

fn target(state: &State<'_>, station: u32, cargo: u8, pickup: u64) -> Result<i64, WorldTickError> {
    let goods = write::goods(state, station, cargo)?;
    let mut rating = 0_i64;
    let speed = i64::try_from(goods.number("last_speed")?)
        .map_err(|_| unsupported("station_rating", "speed"))?
        .wrapping_sub(85);
    if speed >= 0 {
        rating = rating.wrapping_add(speed >> 2);
    }
    for (limit, points) in [(21, 25), (12, 25), (6, 45), (3, 35)] {
        if pickup <= limit {
            rating = rating.wrapping_add(points);
        }
    }
    rating = rating.wrapping_sub(90);
    let max_waiting = goods.number("max_waiting_cargo")?;
    for (limit, points) in [(1500, 55), (1000, 35), (600, 10), (300, 20), (100, 10)] {
        if max_waiting <= limit {
            rating = rating.wrapping_add(points);
        }
    }
    let age = goods.number("last_age")?;
    for (limit, points) in [(3, 10), (2, 10), (1, 13)] {
        if age < limit {
            rating = rating.wrapping_add(points);
        }
    }
    let base = Read::Candidate(state.view())
        .row(*b"STNN", station)?
        .child("normal")?
        .child("base")?;
    let owner = base.number("owner")?;
    let town = u32::try_from(
        base.number("town")?
            .checked_sub(1)
            .ok_or(RuntimeError::Invalid("rating town"))?,
    )
    .map_err(|_| unsupported("station_rating", "town id"))?;
    if owner < 15 && state.unsigned(b"CITY", town, "statues")? & (1_u64 << owner) != 0 {
        rating = rating.wrapping_add(26);
    }
    Ok(rating)
}

fn increment_timers(state: &mut State<'_>, station: u32) -> Result<(), WorldTickError> {
    for name in ["time_since_load", "time_since_unload"] {
        let old = Read::Candidate(state.view())
            .row(*b"STNN", station)?
            .child("normal")?
            .number(name)?;
        normal(state, station, name, old.saturating_add(1).min(255))?;
    }
    Ok(())
}
