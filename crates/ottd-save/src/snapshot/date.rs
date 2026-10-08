use super::{FieldValue, SnapshotError, invalid, required, table};
use crate::Savegame;
use serde::{Deserialize, Serialize};

/// All current DATE state using upstream field names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DateState {
    date: i32,
    date_fract: u16,
    tick_counter: u64,
    economy_date: i32,
    economy_date_fract: u16,
    days_since_last_month: u32,
    calendar_sub_date_fract: u16,
    cur_tileloop_tile: u32,
    next_disaster_start: u16,
    random_state: [u32; 2],
    company_tick_counter: u8,
    trees_tick_counter: u8,
    pause_mode: u8,
    id: String,
    competitors_interval: u32,
    competitors_interval_elapsed: u32,
    competitors_interval_fired: i8,
}

impl DateState {
    /// Saved `date`.
    pub const fn date(&self) -> i32 {
        self.date
    }
    /// Saved `date_fract`.
    pub const fn date_fract(&self) -> u16 {
        self.date_fract
    }
    /// Saved `tick_counter`.
    pub const fn tick_counter(&self) -> u64 {
        self.tick_counter
    }
    /// Saved `economy_date`.
    pub const fn economy_date(&self) -> i32 {
        self.economy_date
    }
    /// Saved `economy_date_fract`.
    pub const fn economy_date_fract(&self) -> u16 {
        self.economy_date_fract
    }
    /// Saved `days_since_last_month`.
    pub const fn days_since_last_month(&self) -> u32 {
        self.days_since_last_month
    }
    /// Saved `calendar_sub_date_fract`.
    pub const fn calendar_sub_date_fract(&self) -> u16 {
        self.calendar_sub_date_fract
    }
    /// Saved `cur_tileloop_tile`.
    pub const fn cur_tileloop_tile(&self) -> u32 {
        self.cur_tileloop_tile
    }
    /// Saved `next_disaster_start`.
    pub const fn next_disaster_start(&self) -> u16 {
        self.next_disaster_start
    }
    /// Saved `random_state`.
    pub const fn random_state(&self) -> [u32; 2] {
        self.random_state
    }
    /// Saved `company_tick_counter`.
    pub const fn company_tick_counter(&self) -> u8 {
        self.company_tick_counter
    }
    /// Saved `trees_tick_counter`.
    pub const fn trees_tick_counter(&self) -> u8 {
        self.trees_tick_counter
    }
    /// Saved `pause_mode`.
    pub const fn pause_mode(&self) -> u8 {
        self.pause_mode
    }
    /// Saved `id`.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Saved `competitors_interval`.
    pub const fn competitors_interval(&self) -> u32 {
        self.competitors_interval
    }
    /// Saved `competitors_interval_elapsed`.
    pub const fn competitors_interval_elapsed(&self) -> u32 {
        self.competitors_interval_elapsed
    }
    /// Saved `competitors_interval_fired`.
    pub const fn competitors_interval_fired(&self) -> i8 {
        self.competitors_interval_fired
    }
}

const SCHEMA: &[(&str, u8)] = &[
    ("date", 5),
    ("date_fract", 4),
    ("tick_counter", 8),
    ("economy_date", 5),
    ("economy_date_fract", 4),
    ("days_since_last_month", 6),
    ("calendar_sub_date_fract", 4),
    ("cur_tileloop_tile", 6),
    ("next_disaster_start", 4),
    ("random_state[0]", 6),
    ("random_state[1]", 6),
    ("company_tick_counter", 2),
    ("trees_tick_counter", 2),
    ("pause_mode", 2),
    ("id", 26),
    ("competitors_interval", 6),
    ("competitors_interval_elapsed", 6),
    ("competitors_interval_fired", 1),
];
pub(super) fn decode(save: &Savegame) -> Result<DateState, SnapshotError> {
    let mut fields = table::single(required(save, *b"DATE")?, SCHEMA)?;
    let state: [u32; 2] = [
        table::unsigned(&fields, "random_state[0]")?,
        table::unsigned(&fields, "random_state[1]")?,
    ];
    fields.remove("random_state[0]");
    fields.remove("random_state[1]");
    fields.insert(
        "random_state".into(),
        FieldValue::Array(
            state
                .into_iter()
                .map(|n| FieldValue::Unsigned(u64::from(n)))
                .collect(),
        ),
    );
    let value = serde_json::to_value(fields).map_err(|error| invalid(error.to_string()))?;
    serde_json::from_value(value).map_err(|error| invalid(error.to_string()))
}
