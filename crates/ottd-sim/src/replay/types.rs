use super::ReplayError;
use crate::CommandRequest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One strictly ordered operation in the native replay protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplayAction {
    /// Execute one native command (native failures do not abort the replay).
    Command {
        /// External ordinal, independent of the saved tick.
        ordinal: u64,
        /// Native command arguments and actor.
        request: CommandRequest,
    },
    /// Execute complete supported native state-loop calls.
    Tick {
        /// External ordinal.
        ordinal: u64,
        /// Requested calls, including paused calls.
        count: u32,
    },
    /// Observe a save/checkpoint boundary without advancing time.
    Checkpoint {
        /// External ordinal.
        ordinal: u64,
        /// Unique ASCII alphanumeric, hyphen or underscore label.
        label: String,
    },
}
impl ReplayAction {
    /// Position in the external scheduling order.
    pub const fn ordinal(&self) -> u64 {
        match self {
            Self::Command { ordinal, .. }
            | Self::Tick { ordinal, .. }
            | Self::Checkpoint { ordinal, .. } => *ordinal,
        }
    }
}
/// Exact native replay input; preparation-only fixture recipes are not accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayPlan {
    /// Supported protocol version, currently one.
    pub schema_version: u32,
    /// Ordered actions, at most 10,000 and 100,000 aggregate tick calls.
    pub actions: Vec<ReplayAction>,
}
/// Persistent scheduling state outside native save chunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayCursor {
    plan: ReplayPlan,
    position: u32,
    next_ordinal: u64,
}
impl ReplayCursor {
    /// Validate a new complete replay plan.
    /// # Errors
    /// Rejects ordering, labels, schema and aggregate resource limits.
    pub fn new(plan: ReplayPlan) -> Result<Self, ReplayError> {
        let cursor = Self {
            plan,
            position: 0,
            next_ordinal: 0,
        };
        cursor.validate()?;
        Ok(cursor)
    }
    /// Validate a deserialized cursor and the original full-plan budgets.
    /// # Errors
    /// Rejects inconsistent cursor state or any malformed action, including past actions.
    pub fn validate(&self) -> Result<(), ReplayError> {
        if self.plan.schema_version != 1 || self.plan.actions.len() > 10_000 {
            return Err(ReplayError::Invalid("schema version or action limit"));
        }
        let position =
            usize::try_from(self.position).map_err(|_| ReplayError::Invalid("cursor position"))?;
        if position > self.plan.actions.len() {
            return Err(ReplayError::Invalid("cursor position"));
        }
        let mut previous = None;
        let mut ticks = 0_u32;
        let mut labels = BTreeSet::new();
        for action in &self.plan.actions {
            let ordinal = action.ordinal();
            if ordinal == u64::MAX || previous.is_some_and(|p| ordinal <= p) {
                return Err(ReplayError::Invalid(
                    "ordinals must increase and leave room for the next ordinal",
                ));
            }
            previous = Some(ordinal);
            match action {
                ReplayAction::Command { request, .. } => {
                    let text = match &request.command {
                        crate::Command::RenameCompany { text }
                        | crate::Command::RenamePresident { text } => Some(text),
                        _ => None,
                    };
                    if text.is_some_and(|t| t.len() > 65_536) {
                        return Err(ReplayError::Invalid("command text byte limit"));
                    }
                }
                ReplayAction::Tick { count, .. } => {
                    ticks = ticks
                        .checked_add(*count)
                        .ok_or(ReplayError::Invalid("tick budget overflow"))?;
                    if ticks > 100_000 {
                        return Err(ReplayError::Invalid("tick limit"));
                    }
                }
                ReplayAction::Checkpoint { label, .. } => {
                    if label.is_empty()
                        || label.len() > 128
                        || label.eq_ignore_ascii_case("initial")
                        || label.eq_ignore_ascii_case("final")
                        || !label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                        || !labels.insert(label.to_ascii_lowercase())
                    {
                        return Err(ReplayError::Invalid(
                            "checkpoint labels must be safe, unique and not reserved",
                        ));
                    }
                }
            }
        }
        let expected = position
            .checked_sub(1)
            .and_then(|index| self.plan.actions.get(index))
            .map_or(0, |action| action.ordinal().saturating_add(1));
        if self.next_ordinal != expected {
            return Err(ReplayError::Invalid(
                "cursor next ordinal does not match completed history",
            ));
        }
        Ok(())
    }
    /// First ordinal that can execute after the completed history.
    pub const fn next_ordinal(&self) -> u64 {
        self.next_ordinal
    }
    /// Number of completed actions, retained across restarts.
    pub const fn position(&self) -> u32 {
        self.position
    }
    /// Remaining actions in the validated plan.
    pub fn remaining(&self) -> usize {
        self.plan
            .actions
            .len()
            .saturating_sub(usize::try_from(self.position).unwrap_or(usize::MAX))
    }
    pub(super) fn next(&self) -> Option<&ReplayAction> {
        usize::try_from(self.position)
            .ok()
            .and_then(|p| self.plan.actions.get(p))
    }
    pub(super) fn advance(&mut self) -> Result<(), ReplayError> {
        let ordinal = self
            .next()
            .ok_or(ReplayError::Invalid("cursor exhausted"))?
            .ordinal();
        self.next_ordinal = ordinal
            .checked_add(1)
            .ok_or(ReplayError::Invalid("ordinal overflow"))?;
        self.position = self
            .position
            .checked_add(1)
            .ok_or(ReplayError::Invalid("cursor overflow"))?;
        Ok(())
    }
}
