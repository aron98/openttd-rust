use crate::{CalendarDate, DateFraction, EconomyDate, TickCounter};

/// Economy calendar interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimekeepingUnits {
    /// Gregorian months and years.
    Calendar,
    /// Thirty-day months and 360-day years.
    Wallclock,
}
/// Validated normal-game clock settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSettings {
    pub(crate) units: TimekeepingUnits,
    pub(crate) minutes: u16,
}
/// Invalid or unsupported clock input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ClockError {
    /// The settings cannot be selected in the original game.
    #[error("calendar units require 12 minutes/year; wallclock requires 0 or 12..=10080")]
    Settings,
    /// Day counts must fit the selected calendar and fractions must be below 74.
    #[error("clock date or fraction is outside the supported range")]
    Date,
    /// Cached year/month do not represent an attainable clock state.
    #[error("clock cache is inconsistent with the dates")]
    Cache,
}
impl ClockSettings {
    /// Checks the original game's selectable settings.
    /// # Errors
    /// Returns [`ClockError::Settings`] for unattainable combinations.
    pub fn new(units: TimekeepingUnits, minutes: u16) -> Result<Self, ClockError> {
        let valid = match units {
            TimekeepingUnits::Calendar => minutes == 12,
            TimekeepingUnits::Wallclock => minutes == 0 || (12..=10080).contains(&minutes),
        };
        if valid {
            Ok(Self { units, minutes })
        } else {
            Err(ClockError::Settings)
        }
    }
    /// Economy date interpretation.
    pub const fn units(self) -> TimekeepingUnits {
        self.units
    }
    /// Calendar progression speed; zero freezes the calendar.
    pub const fn minutes_per_calendar_year(self) -> u16 {
        self.minutes
    }
}
/// Saved DATE fields consumed by the clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSnapshot {
    /// Calendar date.
    pub date: CalendarDate,
    /// Calendar ticks within the day.
    pub date_fract: DateFraction,
    /// Accumulated slow-calendar ticks.
    pub calendar_sub_date_fract: u16,
    /// Economy date in the configured units.
    pub economy_date: EconomyDate,
    /// Economy ticks within the day.
    pub economy_date_fract: DateFraction,
    /// Days since the previous economy month callback.
    pub days_since_last_month: u32,
    /// Simulation ticks.
    pub tick_counter: TickCounter,
}
/// Runtime caches, including the original wallclock maximum-year rewind quirk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockCache {
    /// Cached calendar year.
    pub calendar_year: i32,
    /// Cached zero-based calendar month.
    pub calendar_month: u8,
    /// Cached economy year.
    pub economy_year: i32,
    /// Cached zero-based economy month.
    pub economy_month: u8,
}
/// Timer boundary notification; callback bodies are not executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockEvent {
    /// Calendar day boundary.
    CalendarDay,
    /// Calendar month boundary.
    CalendarMonth,
    /// Calendar year boundary.
    CalendarYear,
    /// Economy day boundary.
    EconomyDay,
    /// Economy week boundary.
    EconomyWeek,
    /// Economy month boundary.
    EconomyMonth,
    /// Economy quarter boundary.
    EconomyQuarter,
    /// Economy year boundary.
    EconomyYear,
}
impl ClockEvent {
    /// Stable boundary name for reports.
    pub const fn name(self) -> &'static str {
        match self {
            Self::CalendarDay => "calendar_day",
            Self::CalendarMonth => "calendar_month",
            Self::CalendarYear => "calendar_year",
            Self::EconomyDay => "economy_day",
            Self::EconomyWeek => "economy_week",
            Self::EconomyMonth => "economy_month",
            Self::EconomyQuarter => "economy_quarter",
            Self::EconomyYear => "economy_year",
        }
    }
}
/// Ordered boundary dispatch produced by one normal-game tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClockEvents {
    /// Whether the calendar fraction advanced (upstream calendar `Elapsed` result).
    pub calendar_progressed: bool,
    pub(crate) boundaries: [bool; 8],
}
impl ClockEvents {
    /// Iterates the original calendar-before-economy callback order.
    pub fn iter(self) -> impl Iterator<Item = ClockEvent> {
        use ClockEvent::{
            CalendarDay, CalendarMonth, CalendarYear, EconomyDay, EconomyMonth, EconomyQuarter,
            EconomyWeek, EconomyYear,
        };
        [
            CalendarDay,
            CalendarMonth,
            CalendarYear,
            EconomyDay,
            EconomyWeek,
            EconomyMonth,
            EconomyQuarter,
            EconomyYear,
        ]
        .into_iter()
        .zip(self.boundaries)
        .filter_map(|(event, active)| active.then_some(event))
    }
}
