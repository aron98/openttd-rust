//! Process-owned native company rating slots; no save-derived reconstruction.
use super::RuntimeError;
use serde::Serialize;
#[cfg(test)]
mod tests;

/// Native `_score_part`: retained between callbacks, including inactive owner slots.
/// Saves do not determine this cache; same-process reload would retain the old slots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CompanyRatingCaches {
    parts: [[i64; 10]; 15],
}
impl CompanyRatingCaches {
    /// Initialize the zeroed native process-global array for an independent runtime.
    pub const fn fresh_process() -> Self {
        Self {
            parts: [[0; 10]; 15],
        }
    }
    /// All ten raw components for every native company slot, including inactive slots.
    pub const fn parts(&self) -> &[[i64; 10]; 15] {
        &self.parts
    }
    pub(crate) fn update(&mut self, owner: u32, parts: [i64; 10]) -> Result<(), RuntimeError> {
        let index = usize::try_from(owner).map_err(|_| RuntimeError::Invalid("rating owner"))?;
        let target = self
            .parts
            .get_mut(index)
            .ok_or(RuntimeError::Invalid("rating owner"))?;
        *target = parts;
        Ok(())
    }
}
