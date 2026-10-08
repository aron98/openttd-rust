use crate::Map;
use serde::{Deserialize, Serialize};

/// Explicit assumptions of this isolated subsystem, not a saved full game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// Only `temperate` is supported.
    pub landscape: String,
    /// Only `normal` gameplay is supported.
    pub mode: String,
    /// Must be false; `NewGRF` callbacks are outside this subsystem.
    pub ambient_callbacks: bool,
    /// Paused calls leave both clocks and terrain unchanged.
    pub paused: bool,
    /// Upstream units: 0 calendar, 1 wallclock.
    pub timekeeping_units: u8,
    /// Calendar speed; 0 freezes the calendar in wallclock mode.
    pub minutes_per_calendar_year: u16,
}

/// Raw clock state including the cached dates required for exact restoration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    /// Gregorian day number.
    pub date: i32,
    /// Calendar day fraction.
    pub date_fract: u16,
    /// Calendar slow-clock accumulator.
    pub calendar_sub_date_fract: u16,
    /// Economy day number.
    pub economy_date: i32,
    /// Economy day fraction.
    pub economy_date_fract: u16,
    /// Economy days since the month boundary.
    pub days_since_last_month: u32,
    /// Game ticks elapsed.
    pub tick_counter: u64,
    /// Cached calendar year.
    pub calendar_year: i32,
    /// Cached calendar month, zero based.
    pub calendar_month: u8,
    /// Cached economy year.
    pub economy_year: i32,
    /// Cached economy month, zero based.
    pub economy_month: u8,
}

/// Mutable state serialized at a subsystem boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// Raw map fields.
    pub map: Map,
    /// Calendar and economy state.
    pub clock: Clock,
    /// Next nonzero tile index.
    pub cur_tileloop_tile: u32,
    /// Gameplay randomizer, unchanged by supported terrain procedures.
    pub random_state: [u32; 2],
}

/// Ordered clock notifications for one requested tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickEvents {
    /// Game tick after the call; unchanged while paused.
    pub tick_counter: u64,
    /// Whether the calendar clock advanced on this call.
    pub calendar_progressed: bool,
    /// Upstream boundary event names in dispatch order.
    pub events: Vec<String>,
}

/// Versioned input/output contract for isolated landscape simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// Currently 1.
    pub schema_version: u32,
    /// Explicit simulation context, validated before any advancement.
    pub context: Context,
    /// Complete supported state, including all raw terrain fields.
    pub state: State,
    /// Trace of newly requested ticks; prior traces are discarded on resumption.
    pub events: Vec<TickEvents>,
}
