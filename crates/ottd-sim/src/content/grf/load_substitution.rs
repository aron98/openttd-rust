use super::{
    load::{ActionResult, Session},
    load_cursor::Cursor,
    load_types::{ControlLoadError, LoadEvent, LoadLocation},
    records::Reader,
};
use std::sync::Arc;

struct Target {
    key: (u32, u32),
    length: usize,
}

impl Session<'_, '_> {
    pub(super) fn substitute(
        &mut self,
        reader: &mut Reader<'_>,
        cursor: &Cursor<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let Some(Target { key, length }) = self.preload(cursor, location)? else {
            return Ok(());
        };
        loop {
            let parameter = reader.byte()?;
            if parameter == 255 {
                break;
            }
            let size = reader.byte()?;
            let add = size & 0x80 != 0;
            let size = u32::from(size & 0x7f);
            let offset = usize::from(reader.extended()?);
            let last = u32::from(parameter).wrapping_add(size.wrapping_sub(1) / 4);
            let count = self
                .registry
                .file(location.file)
                .map_or(0, |file| file.parameters.len());
            if parameter < 0x80 && u64::from(last) >= u64::try_from(count).unwrap_or(u64::MAX) {
                break;
            }
            let mut carry = false;
            for index in 0..size {
                let position = offset.saturating_add(usize::try_from(index).unwrap_or(usize::MAX));
                if position >= length {
                    break;
                }
                let parameter = u32::from(parameter).wrapping_add(index / 4).to_le_bytes()[0];
                let value = self.parameter(parameter, location, 6)?;
                let byte = (value >> ((index % 4).saturating_mul(8))).to_le_bytes()[0];
                if index % 4 == 0 {
                    carry = false;
                }
                self.budget.overrides(1, location)?;
                if let Some(bytes) = self.overrides.get_mut(&key) {
                    if let Some(target) = Arc::make_mut(bytes).get_mut(position) {
                        if add {
                            let sum = u16::from(*target)
                                .saturating_add(u16::from(byte))
                                .saturating_add(u16::from(carry));
                            *target = sum.to_le_bytes()[0];
                            carry = sum >= 256;
                        } else {
                            *target = byte;
                        }
                    }
                }
            }
        }
        self.budget.trace(length, location)?;
        let bytes = self
            .overrides
            .get(&key)
            .map_or_else(Vec::new, |bytes| bytes.to_vec());
        self.events.push(LoadEvent::Override {
            location,
            target_line: key.1,
            bytes,
        });
        Ok(())
    }
    fn preload(
        &mut self,
        cursor: &Cursor<'_>,
        location: LoadLocation,
    ) -> ActionResult<Option<Target>> {
        let mut next = Reader {
            bytes: cursor.reader.bytes,
            pos: cursor.reader.pos,
        };
        let length = (if cursor.version >= 2 {
            next.dword()
        } else {
            next.word().map(u32::from)
        })
        .map_err(|source| ControlLoadError::Structural {
            file: location.file,
            source,
        })?;
        let kind = next
            .byte()
            .map_err(|_| ControlLoadError::InvalidNativeDomain {
                location,
                detail: "Action6 lookahead exceeds physical source bytes",
            })?;
        if kind != 255 {
            return Ok(None);
        }
        let length =
            usize::try_from(length).map_err(|_| ControlLoadError::InvalidNativeDomain {
                location,
                detail: "target sprite length",
            })?;
        let target_line = location
            .line
            .checked_add(1)
            .ok_or(ControlLoadError::ResourceLimit {
                location,
                resource: "NFO lines",
            })?;
        let grfid = self
            .inputs
            .get(location.file)
            .map_or(0, |input| input.identity.grfid);
        let key = (grfid, target_line);
        if self
            .overrides
            .get(&key)
            .is_none_or(|bytes| bytes.is_empty())
        {
            self.budget.overrides(length, location)?;
            let raw = next
                .take(length)
                .map_err(|source| ControlLoadError::Structural {
                    file: location.file,
                    source,
                })?;
            self.overrides.insert(key, Arc::from(raw));
        }
        if self
            .overrides
            .get(&key)
            .is_some_and(|bytes| bytes.len() != length)
        {
            return Err(ControlLoadError::InvalidNativeDomain {
                location,
                detail: "aliased Action6 target length",
            }
            .into());
        }
        Ok(Some(Target { key, length }))
    }
}
