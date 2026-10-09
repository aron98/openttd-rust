use super::ReplayError;
use crate::world_access::{signed, unsigned};
use ottd_core::CalendarDate;
use ottd_save::world::World;
use serde::{Deserialize, Serialize};

/// Deterministic runtime fields shared by Rust and original-engine observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayRuntime {
    /// Native saved tick counter.
    pub tick: u64,
    /// Native pause-reason bitset.
    pub pause: u64,
    /// Calendar day count.
    pub calendar_date: i32,
    /// Calendar fractional day.
    pub calendar_fract: u64,
    /// Calendar year cache.
    pub calendar_year: i32,
    /// Calendar month cache, zero based.
    pub calendar_month: u8,
    /// Economy day count.
    pub economy_date: i32,
    /// Economy fractional day.
    pub economy_fract: u64,
    /// Economy year cache.
    pub economy_year: i32,
    /// Economy month cache, zero based.
    pub economy_month: u8,
    /// Saved gameplay random state; host interactive RNG is not part of this model.
    pub random: [u64; 2],
}
pub(super) fn observe(world: &World) -> Result<ReplayRuntime, ReplayError> {
    let calendar_date = i32::try_from(signed(world, b"DATE", 0, "date")?)
        .map_err(|_| ReplayError::Unsupported("calendar date range"))?;
    let economy_date = i32::try_from(signed(world, b"DATE", 0, "economy_date")?)
        .map_err(|_| ReplayError::Unsupported("economy date range"))?;
    let (calendar_year, calendar_month, _) = CalendarDate::from_raw(calendar_date)
        .map_err(|_| ReplayError::Unsupported("calendar date range"))?
        .ymd();
    let (economy_year, economy_month) =
        match unsigned(world, b"PATS", 0, "economy.timekeeping_units")? {
            0 => {
                let (year, month, _) = CalendarDate::from_raw(economy_date)
                    .map_err(|_| ReplayError::Unsupported("economy date range"))?
                    .ymd();
                (year, month)
            }
            1 if economy_date >= 0 => (
                economy_date / 360,
                u8::try_from((economy_date % 360) / 30)
                    .map_err(|_| ReplayError::Unsupported("economy month"))?,
            ),
            _ => return Err(ReplayError::Unsupported("timekeeping units")),
        };
    if calendar_year >= 5_000_000 || economy_year >= 5_000_000 {
        return Err(ReplayError::Unsupported("maximum-year runtime cache"));
    }
    Ok(ReplayRuntime {
        tick: unsigned(world, b"DATE", 0, "tick_counter")?,
        pause: unsigned(world, b"DATE", 0, "pause_mode")?,
        calendar_date,
        calendar_fract: unsigned(world, b"DATE", 0, "date_fract")?,
        calendar_year,
        calendar_month,
        economy_date,
        economy_fract: unsigned(world, b"DATE", 0, "economy_date_fract")?,
        economy_year,
        economy_month,
        random: [
            unsigned(world, b"DATE", 0, "random_state[0]")?,
            unsigned(world, b"DATE", 0, "random_state[1]")?,
        ],
    })
}
