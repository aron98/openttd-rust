use crate::clock_types::{
    ClockCache, ClockError, ClockEvents, ClockSettings, ClockSnapshot, TimekeepingUnits,
};
use crate::{CalendarDate, DateFraction, EconomyDate, MAX_YEAR, leap, year_start};

/// Normal-game clock dispatch. Emits boundaries without executing game callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockState {
    saved: ClockSnapshot,
    settings: ClockSettings,
    cache: ClockCache,
}
impl ClockState {
    /// Loads DATE fields, deriving caches as upstream `SetDate` does.
    /// # Errors
    /// Returns [`ClockError::Date`] for invalid day counts or fractions.
    pub fn new(saved: ClockSnapshot, settings: ClockSettings) -> Result<Self, ClockError> {
        let limit = match settings.units {
            TimekeepingUnits::Calendar => year_start(MAX_YEAR.wrapping_add(1)),
            TimekeepingUnits::Wallclock => MAX_YEAR.wrapping_add(1).wrapping_mul(360),
        };
        if saved.date_fract.0 >= 74
            || saved.economy_date_fract.0 >= 74
            || !(0..limit).contains(&saved.economy_date.0)
        {
            return Err(ClockError::Date);
        }
        let (calendar_year, calendar_month, _) = saved.date.ymd();
        let (economy_year, economy_month) = economy_ym(saved.economy_date, settings.units);
        Ok(Self {
            saved,
            settings,
            cache: ClockCache {
                calendar_year,
                calendar_month,
                economy_year,
                economy_month,
            },
        })
    }
    /// Restores runtime caches for exact continuation across maximum-year rewinds.
    /// # Errors
    /// Returns [`ClockError::Date`] for invalid fields or [`ClockError::Cache`] for inconsistent caches.
    pub fn restore(
        saved: ClockSnapshot,
        settings: ClockSettings,
        cache: ClockCache,
    ) -> Result<Self, ClockError> {
        let mut state = Self::new(saved, settings)?;
        let rewind_cache = settings.units == TimekeepingUnits::Wallclock
            && saved.economy_date.0 == MAX_YEAR.wrapping_mul(360).wrapping_sub(6)
            && cache.economy_year == MAX_YEAR
            && cache.economy_month == 0;
        if cache.calendar_year != state.cache.calendar_year
            || cache.calendar_month != state.cache.calendar_month
            || ((cache.economy_year, cache.economy_month) != state.economy_ym() && !rewind_cache)
        {
            return Err(ClockError::Cache);
        }
        state.cache = cache;
        Ok(state)
    }
    /// Current DATE fields.
    pub const fn snapshot(self) -> ClockSnapshot {
        self.saved
    }
    /// Current runtime caches.
    pub const fn cache(self) -> ClockCache {
        self.cache
    }
    /// Current settings.
    pub const fn settings(self) -> ClockSettings {
        self.settings
    }
    /// Cached calendar year and zero-based month.
    pub const fn calendar_ym(self) -> (i32, u8) {
        (self.cache.calendar_year, self.cache.calendar_month)
    }
    /// Cached economy year and zero-based month.
    pub const fn economy_ym(self) -> (i32, u8) {
        (self.cache.economy_year, self.cache.economy_month)
    }
    /// Executes clock dispatch in normal-game order; a paused dispatch changes nothing.
    /// Does not execute vehicles, industries, companies, or any registered timer callbacks.
    pub fn advance(&mut self, paused: bool) -> ClockEvents {
        if paused {
            return ClockEvents::default();
        }
        let (calendar_progressed, calendar) = self.advance_calendar();
        let economy = self.advance_economy();
        self.saved.tick_counter.0 = self.saved.tick_counter.0.wrapping_add(1);
        let [cd, cm, cy] = calendar;
        let [ed, ew, em, eq, ey] = economy;
        ClockEvents {
            calendar_progressed,
            boundaries: [cd, cm, cy, ed, ew, em, eq, ey],
        }
    }
    fn advance_calendar(&mut self) -> (bool, [bool; 3]) {
        if self.settings.minutes == 0 {
            return (false, [false; 3]);
        }
        if self.settings.minutes != 12 {
            self.saved.calendar_sub_date_fract =
                self.saved.calendar_sub_date_fract.wrapping_add(74);
            let threshold = u32::from(self.settings.minutes).wrapping_mul(74) / 12;
            if u32::from(self.saved.calendar_sub_date_fract) < threshold {
                return (false, [false; 3]);
            }
            let remainder = u32::from(self.saved.calendar_sub_date_fract)
                .wrapping_sub(threshold)
                .min(73);
            let [low, high, _, _] = remainder.to_le_bytes();
            self.saved.calendar_sub_date_fract = u16::from_le_bytes([low, high]);
        }
        self.saved.date_fract.0 = self.saved.date_fract.0.wrapping_add(1);
        if self.saved.date_fract.0 < 74 {
            return (true, [false; 3]);
        }
        self.saved.date_fract = DateFraction(0);
        self.saved.calendar_sub_date_fract = 0;
        let raw = self.saved.date.raw().wrapping_add(1);
        let wrap = raw == year_start(MAX_YEAR.wrapping_add(1));
        self.saved.date = CalendarDate(if wrap { year_start(MAX_YEAR) } else { raw });
        let (year, month, _) = self.saved.date.ymd();
        let boundaries = [
            true,
            month != self.cache.calendar_month,
            wrap || year != self.cache.calendar_year,
        ];
        self.cache.calendar_year = year;
        self.cache.calendar_month = month;
        (true, boundaries)
    }
    fn advance_economy(&mut self) -> [bool; 5] {
        self.saved.economy_date_fract.0 = self.saved.economy_date_fract.0.wrapping_add(1);
        if self.saved.economy_date_fract.0 < 74 {
            return [false; 5];
        }
        self.saved.economy_date_fract = DateFraction(0);
        self.saved.economy_date.0 = self.saved.economy_date.0.wrapping_add(1);
        self.saved.days_since_last_month = self.saved.days_since_last_month.wrapping_add(1);
        let (year, month) = economy_ym(self.saved.economy_date, self.settings.units);
        let new_month = month != self.cache.economy_month;
        let boundaries = [
            true,
            self.saved.economy_date.0 % 7 == 3,
            new_month,
            new_month && month % 3 == 0,
            year != self.cache.economy_year,
        ];
        self.cache.economy_year = year;
        self.cache.economy_month = month;
        if new_month {
            self.saved.days_since_last_month = 0;
        }
        if year == MAX_YEAR.wrapping_add(1) {
            self.cache.economy_year = MAX_YEAR;
            self.saved.economy_date.0 =
                self.saved
                    .economy_date
                    .0
                    .wrapping_sub(if leap(MAX_YEAR) { 366 } else { 365 });
        }
        boundaries
    }
}
fn economy_ym(date: EconomyDate, units: TimekeepingUnits) -> (i32, u8) {
    match units {
        TimekeepingUnits::Calendar => {
            if date.0 == year_start(MAX_YEAR.wrapping_add(1)) {
                return (MAX_YEAR.wrapping_add(1), 0);
            }
            let (year, month, _) = CalendarDate(date.0).ymd();
            (year, month)
        }
        TimekeepingUnits::Wallclock => {
            let [month, ..] = ((date.0 % 360) / 30).to_le_bytes();
            (date.0 / 360, month)
        }
    }
}
