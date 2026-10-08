//! Typed version-362 state, without historical migration or simulation.
mod date;
mod map;
mod model;
mod schema;
mod table;

pub use date::DateState;
pub use map::{MapState, TileState};
pub use model::{FieldValue, ScriptRandomState, WorldSnapshot};

use crate::{Chunk, Savegame};

/// Failure to decode the supported typed state boundary.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// The lossless codec accepts this version, but typed migration is not implemented.
    #[error("typed snapshots require save version 362, got {0}")]
    Version(u16),
    /// A required chunk is absent or appears twice.
    #[error("required chunk {0} must appear exactly once")]
    Chunk(String),
    /// A table, field, map plane, or JSON snapshot violates its schema.
    #[error("invalid snapshot: {0}")]
    Invalid(String),
    /// Container framing inside the selected chunk is invalid.
    #[error(transparent)]
    Container(#[from] crate::Error),
}

impl Savegame {
    /// Decode map, clocks, settings, and script randomness from version 362.
    ///
    /// # Errors
    /// Rejects older versions, missing/duplicate chunks, and malformed typed data.
    pub fn snapshot(&self) -> Result<WorldSnapshot, SnapshotError> {
        if self.version() != crate::SAVEGAME_VERSION {
            return Err(SnapshotError::Version(self.version()));
        }
        let map = map::decode(self)?;
        let date = date::decode(self)?;
        let settings = table::single(required(self, *b"PATS")?, schema::SETTINGS)?;
        let random = table::decode(required(self, *b"SRND")?, schema::RANDOM)?;
        let script_random = random
            .into_iter()
            .map(|(owner, fields)| {
                Ok(ScriptRandomState::new(
                    owner,
                    [
                        table::unsigned(&fields, "state[0]")?,
                        table::unsigned(&fields, "state[1]")?,
                    ],
                ))
            })
            .collect::<Result<_, SnapshotError>>()?;
        WorldSnapshot::new(map, date, settings, script_random)
    }
}

fn required(save: &Savegame, id: [u8; 4]) -> Result<&Chunk, SnapshotError> {
    let mut found = save.chunks().iter().filter(|chunk| chunk.id() == id);
    let chunk = found.next();
    if found.next().is_some() {
        return Err(SnapshotError::Chunk(
            String::from_utf8_lossy(&id).into_owned(),
        ));
    }
    chunk.ok_or_else(|| SnapshotError::Chunk(String::from_utf8_lossy(&id).into_owned()))
}

fn invalid(message: impl Into<String>) -> SnapshotError {
    SnapshotError::Invalid(message.into())
}
