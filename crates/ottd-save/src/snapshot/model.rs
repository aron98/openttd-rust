use super::{DateState, MapState, SnapshotError, invalid, table};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A saved settings value; integer signedness follows the wire descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    /// Signed wire integer.
    Signed(i64),
    /// Unsigned wire integer.
    Unsigned(u64),
    /// UTF-8 string.
    String(String),
    /// Ordered primitive array.
    Array(Vec<Self>),
}

/// The saved randomizer state for a script owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptRandomState {
    owner: u32,
    state: [u32; 2],
}

impl ScriptRandomState {
    pub(super) const fn new(owner: u32, state: [u32; 2]) -> Self {
        Self { owner, state }
    }
    /// Upstream owner index.
    pub const fn owner(&self) -> u32 {
        self.owner
    }
    /// Both saved generator words.
    pub const fn state(&self) -> [u32; 2] {
        self.state
    }
}

/// Immutable typed state shared by the native oracle and Rust decoder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "WorldWire")]
pub struct WorldSnapshot {
    schema_version: u32,
    savegame_version: u16,
    map: MapState,
    date: DateState,
    settings: BTreeMap<String, FieldValue>,
    script_random: Vec<ScriptRandomState>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldWire {
    schema_version: u32,
    savegame_version: u16,
    map: MapState,
    date: DateState,
    settings: BTreeMap<String, FieldValue>,
    script_random: Vec<ScriptRandomState>,
}

impl TryFrom<WorldWire> for WorldSnapshot {
    type Error = SnapshotError;
    fn try_from(raw: WorldWire) -> Result<Self, Self::Error> {
        if raw.schema_version != 1 {
            return Err(invalid("unsupported snapshot schema version"));
        }
        if raw.savegame_version != crate::SAVEGAME_VERSION {
            return Err(SnapshotError::Version(raw.savegame_version));
        }
        Self::new(raw.map, raw.date, raw.settings, raw.script_random)
    }
}

impl WorldSnapshot {
    pub(super) fn new(
        map: MapState,
        date: DateState,
        mut settings: table::Fields,
        script_random: Vec<ScriptRandomState>,
    ) -> Result<Self, SnapshotError> {
        table::validate_settings(&mut settings)?;
        if script_random.len() != 19
            || script_random
                .iter()
                .zip(0..19)
                .any(|(record, owner)| record.owner != owner)
        {
            return Err(invalid(
                "SRND must contain owners 0 through 18 exactly once in order",
            ));
        }
        if !(0..=1).contains(&date.competitors_interval_fired()) {
            return Err(invalid("DATE competitor fired flag must be 0 or 1"));
        }
        Ok(Self {
            schema_version: 1,
            savegame_version: crate::SAVEGAME_VERSION,
            map,
            date,
            settings,
            script_random,
        })
    }
    /// Canonical JSON schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Native save format version.
    pub const fn savegame_version(&self) -> u16 {
        self.savegame_version
    }
    /// Dimensions and raw tile state.
    pub const fn map(&self) -> &MapState {
        &self.map
    }
    /// Calendar, economy, ticks and global randomizer state.
    pub const fn date(&self) -> &DateState {
        &self.date
    }
    /// Every PATS field keyed by its upstream save name.
    pub const fn settings(&self) -> &BTreeMap<String, FieldValue> {
        &self.settings
    }
    /// Script randomizers in saved owner order.
    pub fn script_random(&self) -> &[ScriptRandomState] {
        &self.script_random
    }
}
