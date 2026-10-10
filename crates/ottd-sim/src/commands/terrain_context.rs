#[cfg(test)]
mod tests;
#[cfg(test)]
pub(super) mod trace;
use super::CommandError;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct TerrainContext {
    pub(super) company: u8,
    depth: u32,
    pub(super) ratings: BTreeMap<u16, i32>,
    #[cfg(test)]
    pub(super) events: Vec<trace::Recorded>,
}
impl TerrainContext {
    pub(super) fn new(company: u8) -> Self {
        Self {
            company,
            ..Self::default()
        }
    }
    pub(super) const fn testing(&self) -> bool {
        self.depth > 0
    }
    pub(super) fn test<T>(
        &mut self,
        action: impl FnOnce(&mut Self) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        if self.depth == 0 {
            self.ratings.clear();
        }
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(CommandError::Overflow("town rating scope"))?;
        #[cfg(test)]
        self.record(trace::Event::ScopeEnter {
            company: self.company,
            depth: self.depth,
            test_mode: self.testing(),
            map_entries: self.ratings.len(),
        });
        let result = action(self);
        self.depth = self
            .depth
            .checked_sub(1)
            .ok_or(CommandError::Overflow("town rating scope"))?;
        #[cfg(test)]
        self.record(trace::Event::ScopeLeave {
            company: self.company,
            depth: self.depth,
            test_mode: self.testing(),
            map_entries: self.ratings.len(),
        });
        result
    }
    #[cfg(test)]
    pub(super) fn phase(&mut self, phase: Phase) {
        self.record(trace::Event::PhaseComplete { phase });
    }
    #[cfg(test)]
    pub(super) fn record(&mut self, event: trace::Event) {
        self.events.push(trace::Recorded {
            ordinal: self.events.len(),
            event,
        });
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq, serde::Serialize))]
#[cfg_attr(test, serde(rename_all = "snake_case"))]
pub(super) enum Phase {
    Test,
    Exec,
    Result,
}

#[derive(Clone, Copy)]
pub(super) struct TerrainFlags(pub(super) u16);
impl TerrainFlags {
    pub(super) const fn executing(self) -> bool {
        self.0 & 1 != 0
    }
    pub(super) const fn automatic(self) -> bool {
        self.0 & 2 != 0
    }
    pub(super) const fn suppress_rating(self) -> bool {
        self.0 & 512 != 0
    }
}
#[derive(Clone, Copy)]
pub(super) struct ClearRequest {
    pub(super) tile: u32,
    pub(super) flags: TerrainFlags,
}
