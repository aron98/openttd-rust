use super::{
    RecordKind,
    load::{ActionError, Session},
    load_cursor::{Cursor, Skip},
    load_types::{ControlLoadError, LoadEvent, LoadFailure, LoadLocation},
    records::Reader,
};
impl Session<'_, '_> {
    fn decision(
        &mut self,
        cursor: &Cursor<'_>,
        location: LoadLocation,
    ) -> Result<(), ControlLoadError> {
        self.budget.trace(self.snapshot_bytes(), location)?;
        self.events.push(LoadEvent::Decision {
            location,
            skip: cursor.skip.value(),
            next_line: cursor.line,
            next_offset: cursor.reader.pos,
            files: self.registry.snapshots(),
            overrides: self.override_snapshot(),
        });
        Ok(())
    }
    pub(super) fn run_file(&mut self, location: LoadLocation) -> Result<(), ControlLoadError> {
        let input = self.inputs.get(location.file).copied().ok_or(
            ControlLoadError::InvalidNativeDomain {
                location,
                detail: "missing config",
            },
        )?;
        let Some(bytes) = input.bytes else {
            return Ok(());
        };
        let mut cursor = Cursor::new(bytes).map_err(|source| ControlLoadError::Structural {
            file: location.file,
            source,
        })?;
        loop {
            let start = cursor.reader.pos;
            let Some((length, kind)) =
                cursor
                    .reader
                    .record_header(cursor.version)
                    .map_err(|source| ControlLoadError::Structural {
                        file: location.file,
                        source,
                    })?
            else {
                break;
            };
            cursor.line = cursor.line.saturating_add(1);
            let location = LoadLocation {
                line: cursor.line,
                offset: start,
                ..location
            };
            self.budget.step(location)?;
            if cursor.skip == Skip::None && (kind != 255 || length > 1024 * 1024) {
                self.emit(
                    LoadEvent::Record {
                        location,
                        action: None,
                        executed: false,
                    },
                    0,
                    location,
                )?;
                self.disable(location.file, location, Some(LoadFailure::UnexpectedSprite))?;
                cursor.skip = Skip::Stop;
                self.decision(&cursor, location)?;
                break;
            }
            cursor.reader.pos = start;
            let Some(record) = cursor.reader.record(cursor.version).map_err(|source| {
                ControlLoadError::Structural {
                    file: location.file,
                    source,
                }
            })?
            else {
                break;
            };
            if cursor.skip != Skip::None {
                self.emit(
                    LoadEvent::Record {
                        location,
                        action: None,
                        executed: false,
                    },
                    0,
                    location,
                )?;
                cursor.skipped();
                self.decision(&cursor, location)?;
                continue;
            }
            let RecordKind::Pseudo(raw) = record.kind else {
                return Err(ControlLoadError::InvalidNativeDomain {
                    location,
                    detail: "unexpected dispatched real sprite",
                });
            };
            let stop = self.execute(raw, input.identity.grfid, &mut cursor, location)?;
            self.decision(&cursor, location)?;
            if stop {
                break;
            }
        }
        Ok(())
    }
    fn execute(
        &mut self,
        raw: &[u8],
        grfid: u32,
        cursor: &mut Cursor<'_>,
        location: LoadLocation,
    ) -> Result<bool, ControlLoadError> {
        let override_bytes = self.overrides.get(&(grfid, location.line)).cloned();
        let payload = override_bytes.as_deref().unwrap_or(raw);
        if payload.len() != raw.len() {
            return Err(ControlLoadError::InvalidNativeDomain {
                location,
                detail: "aliased Action6 override length",
            });
        }
        self.emit(
            LoadEvent::Record {
                location,
                action: payload.first().copied(),
                executed: true,
            },
            0,
            location,
        )?;
        let mut reader = Reader {
            bytes: payload,
            pos: 0,
        };
        match self.dispatch(&mut reader, cursor, location) {
            Ok(()) => (),
            Err(ActionError::Bounds) => {
                self.disable(location.file, location, Some(LoadFailure::ReadBounds))?;
                cursor.skip = Skip::Stop;
            }
            Err(ActionError::Host(error)) => return Err(error),
        }
        Ok(cursor.skip == Skip::Stop)
    }
}
