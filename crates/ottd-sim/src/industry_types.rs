//! Industry monthly callback state and original callback-phase context.
use serde::{Deserialize, Serialize};

/// Produced/transported history record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducedHistory {
    /// Produced units.
    pub production: u16,
    /// Transported units.
    pub transported: u16,
}
/// Accepted/waiting history record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedHistory {
    /// Accepted units.
    pub accepted: u16,
    /// Average waiting units.
    pub waiting: u16,
}
/// Full produced cargo statistics slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndustryProduced {
    /// Native cargo ID; 255 denotes an invalid slot.
    pub cargo: u8,
    /// Currently waiting output, preserved.
    pub waiting: u16,
    /// Production rate, preserved in original monthly economy.
    pub rate: u8,
    /// All 61 native history records, including current month.
    pub history: Vec<ProducedHistory>,
}
/// Full accepted cargo statistics slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndustryAccepted {
    /// Native cargo ID; 255 denotes an invalid slot.
    pub cargo: u8,
    /// Currently waiting input, preserved.
    pub waiting: u16,
    /// Accrued waiting total; reset only for valid cargo with allocated history.
    pub accumulated_waiting: u32,
    /// Last acceptance date, preserved.
    pub last_accepted: i32,
    /// Optional allocated native history, exactly 61 records when present.
    pub history: Option<Vec<AcceptedHistory>>,
}
/// Statistics and production state of a live original industry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallbackIndustry {
    /// Native industry pool index, below 64000.
    pub id: u16,
    /// Original industry type, below 37.
    pub industry_type: u8,
    /// Production level; zero would require the unported closure branch.
    pub prod_level: u8,
    /// Last economy year with nonzero production.
    pub last_prod_year: i32,
    /// Entire raw validity mask, including preserved bits outside history ranges.
    pub valid_history: u64,
    /// All produced cargo slots.
    pub produced: Vec<IndustryProduced>,
    /// All accepted cargo slots.
    pub accepted: Vec<IndustryAccepted>,
}
/// Explicit phase context before the timer resets its month-day accumulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndustryMonth {
    /// New zero-based month at callback dispatch.
    pub month: u8,
    /// New year at dispatch; may be 5000001 before clock rewind.
    pub year: i32,
    /// Accumulated days before the timer resets this field, possibly zero.
    pub days_since_last_month: u32,
}
/// Supported entire monthly industry state, with explicit original-economy scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndustryCallbacks {
    /// Must be0: original economy; other economies execute random policy.
    pub economy_type: u8,
    /// Must be false: production callbacks and custom specifications are excluded.
    pub newgrf: bool,
    /// Native density0 means fund only; values1..6 permit builder growth.
    pub industry_density: u8,
    /// Native map width.
    pub map_width: u32,
    /// Native map height.
    pub map_height: u32,
    /// Executing company, restored after temporary owner-none scope.
    pub current_company: u8,
    /// Gameplay random state, unchanged within this supported domain.
    pub random_state: [u32; 2],
    /// Builder's16.16 desired industry count.
    pub wanted_inds: u32,
    /// Complete sorted modeled industry pool; type counts derive from these IDs.
    pub industries: Vec<CallbackIndustry>,
}
