//! Deterministic primitives ported from OpenTTD 15.3.

/// The two-word OpenTTD game randomizer; all state is explicitly restorable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Randomizer {
    state: [u32; 2],
}
impl Randomizer {
    /// Initializes both words as upstream `SetSeed` does.
    pub const fn seeded(seed: u32) -> Self {
        Self::from_state([seed, seed])
    }
    /// Restores the exact saved words, including the all-zero state.
    pub const fn from_state(state: [u32; 2]) -> Self {
        Self { state }
    }
    /// Returns the exact state for saving.
    pub const fn state(self) -> [u32; 2] {
        self.state
    }
    /// Advances the generator using upstream's modulo-2^32 arithmetic.
    pub const fn next_u32(&mut self) -> u32 {
        let [s, t] = self.state;
        let value = s.rotate_right(3).wrapping_sub(1);
        self.state = [
            s.wrapping_add((t ^ 0x1234_567f).rotate_right(7))
                .wrapping_add(1),
            value,
        ];
        value
    }
    /// Advances once and scales to `[0, limit)`; a zero limit returns zero.
    pub fn next_bounded(&mut self, limit: u32) -> u32 {
        let product = u64::from(self.next_u32()).wrapping_mul(u64::from(limit));
        let [_, _, _, _, a, b, c, d] = product.to_le_bytes();
        u32::from_le_bytes([a, b, c, d])
    }
}

/// Invalid map dimensions: each axis must be a power of two in 64..=4096.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("map dimensions must be powers of two in 64..=4096")]
pub struct InvalidDimensions;
/// A linear tile index constructed through checked map methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileIndex(u32);
impl TileIndex {
    /// Returns the upstream linear index.
    pub const fn raw(self) -> u32 {
        self.0
    }
}
/// Validated dimensions using upstream allocation limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapDimensions {
    width: u32,
    height: u32,
}
impl MapDimensions {
    /// Checks both dimensions.
    /// # Errors
    /// Returns `InvalidDimensions` for out-of-range or non-power-of-two axes.
    pub fn new(width: u32, height: u32) -> Result<Self, InvalidDimensions> {
        if [width, height]
            .into_iter()
            .all(|n| (64..=4096).contains(&n) && n.is_power_of_two())
        {
            Ok(Self { width, height })
        } else {
            Err(InvalidDimensions)
        }
    }
    /// Width in tiles.
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Height in tiles.
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Number of allocated tiles, including void borders.
    pub const fn tile_count(self) -> u32 {
        self.width.wrapping_mul(self.height)
    }
    /// Checks a linear index against this map.
    pub const fn index(self, raw: u32) -> Option<TileIndex> {
        if raw < self.tile_count() {
            Some(TileIndex(raw))
        } else {
            None
        }
    }
    /// Converts an in-bounds coordinate without masking or wrapping borders.
    pub const fn tile(self, x: u32, y: u32) -> Option<TileIndex> {
        if x < self.width && y < self.height {
            Some(TileIndex(y.wrapping_mul(self.width).wrapping_add(x)))
        } else {
            None
        }
    }
    /// Converts a tile index, rejecting indices from larger maps.
    pub const fn coordinates(self, tile: TileIndex) -> Option<(u32, u32)> {
        if tile.0 < self.tile_count() {
            Some((
                tile.0 & self.width.wrapping_sub(1),
                tile.0 >> self.width.trailing_zeros(),
            ))
        } else {
            None
        }
    }
}

/// Invalid calendar date or a date outside years 0..=5,000,000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid calendar date or outside years 0..=5000000")]
pub struct InvalidDate;
/// Days since January 1, year zero in the proleptic Gregorian calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDate(i32);
const MAX_YEAR: i32 = 5_000_000;
const fn leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
const fn year_start(year: i32) -> i32 {
    year.wrapping_mul(365)
        .wrapping_add(year.wrapping_add(3) / 4)
        .wrapping_sub(year.wrapping_add(99) / 100)
        .wrapping_add(year.wrapping_add(399) / 400)
}
const fn month_days(year: i32, month: u8) -> u8 {
    match month {
        1 => {
            if leap(year) {
                29
            } else {
                28
            }
        }
        3 | 5 | 8 | 10 => 30,
        _ => 31,
    }
}
impl CalendarDate {
    /// Checks a saved calendar day count.
    /// # Errors
    /// Returns `InvalidDate` outside the supported calendar range.
    pub fn from_raw(raw: i32) -> Result<Self, InvalidDate> {
        if (0..year_start(MAX_YEAR.wrapping_add(1))).contains(&raw) {
            Ok(Self(raw))
        } else {
            Err(InvalidDate)
        }
    }
    /// Converts a year, zero-based month and one-based day.
    /// # Errors
    /// Returns `InvalidDate` for invalid components, including nonexistent days.
    pub fn from_ymd(year: i32, month: u8, day: u8) -> Result<Self, InvalidDate> {
        if !(0..=MAX_YEAR).contains(&year)
            || month > 11
            || day == 0
            || day > month_days(year, month)
        {
            return Err(InvalidDate);
        }
        let preceding: i32 = (0..month).map(|m| i32::from(month_days(year, m))).sum();
        Ok(Self(
            year_start(year)
                .wrapping_add(preceding)
                .wrapping_add(i32::from(day))
                .wrapping_sub(1),
        ))
    }
    /// Returns the saved day count.
    pub const fn raw(self) -> i32 {
        self.0
    }
    /// Converts to year, zero-based month and one-based day.
    pub fn ymd(self) -> (i32, u8, u8) {
        let mut low = 0i32;
        let mut high = MAX_YEAR.wrapping_add(1);
        while high.wrapping_sub(low) > 1 {
            let middle = low.wrapping_add(high.wrapping_sub(low) / 2);
            if year_start(middle) <= self.0 {
                low = middle;
            } else {
                high = middle;
            }
        }
        let mut remaining = self.0.wrapping_sub(year_start(low));
        let mut month = 0u8;
        while remaining >= i32::from(month_days(low, month)) {
            remaining = remaining.wrapping_sub(i32::from(month_days(low, month)));
            month = month.wrapping_add(1);
        }
        let [day, ..] = remaining.wrapping_add(1).to_le_bytes();
        (low, month, day)
    }
    /// Adds days, rejecting overflow or departure from the supported range.
    pub fn checked_add_days(self, days: i32) -> Option<Self> {
        Self::from_raw(self.0.checked_add(days)?).ok()
    }
}

/// Saved economy-clock day count; its interpretation depends on timekeeping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EconomyDate(pub i32);
/// Saved sub-day clock fraction, independent of calendar conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateFraction(pub u16);
/// Saved simulation tick counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickCounter(pub u64);

mod clock;
mod clock_types;
pub use clock::ClockState;
pub use clock_types::{
    ClockCache, ClockError, ClockEvent, ClockEvents, ClockSettings, ClockSnapshot, TimekeepingUnits,
};
