use super::types::{GrfParseError as Error, Record, RecordKind};

pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}
impl<'a> Reader<'a> {
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or(Error::Truncated(self.pos))?;
        let value = self
            .bytes
            .get(self.pos..end)
            .ok_or(Error::Truncated(self.pos))?;
        self.pos = end;
        Ok(value)
    }
    pub(super) fn byte(&mut self) -> Result<u8, Error> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(Error::Truncated(self.pos))
    }
    pub(super) fn word(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| Error::Truncated(self.pos))?,
        ))
    }
    pub(super) fn dword(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| Error::Truncated(self.pos))?,
        ))
    }
    pub(super) fn extended(&mut self) -> Result<u16, Error> {
        let value = self.byte()?;
        if value == 255 {
            self.word()
        } else {
            Ok(u16::from(value))
        }
    }
    pub(super) fn string(&mut self) -> Result<&'a [u8], Error> {
        let rest = self
            .bytes
            .get(self.pos..)
            .ok_or(Error::Truncated(self.pos))?;
        let length = rest.iter().position(|&v| v == 0).unwrap_or(rest.len());
        let value = self.take(length)?;
        if self.pos < self.bytes.len() {
            self.byte()?;
        }
        Ok(value)
    }
    pub(super) const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }
    fn length(&mut self, version: u8) -> Result<u32, Error> {
        if version == 1 {
            Ok(u32::from(self.word()?))
        } else {
            self.dword()
        }
    }
    pub(super) fn initial(&mut self, version: u8) -> Result<(), Error> {
        if self.length(version)? != 4 || self.byte()? != 255 {
            return Err(Error::InitialRecord);
        }
        self.dword()?;
        Ok(())
    }
    pub(super) fn record(&mut self, version: u8) -> Result<Option<Record<'a>>, Error> {
        let start = self.pos;
        let length = self.length(version)?;
        if length == 0 {
            return Ok(None);
        }
        let flags = self.byte()?;
        let kind = match flags {
            255 => RecordKind::Pseudo(
                self.take(usize::try_from(length).map_err(|_| Error::ResourceLimit)?)?,
            ),
            253 if version == 2 => RecordKind::SpriteReference(
                self.take(usize::try_from(length).map_err(|_| Error::ResourceLimit)?)?,
            ),
            _ => {
                let payload_start = self.pos;
                let mut decoded = length.checked_sub(8).ok_or(Error::InlineSprite(start))?;
                self.take(7)?;
                if flags & 2 != 0 {
                    self.take(usize::try_from(decoded).map_err(|_| Error::ResourceLimit)?)?;
                } else {
                    while decoded > 0 {
                        let control = i8::from_ne_bytes([self.byte()?]);
                        let count = if control >= 0 {
                            let count = if control == 0 {
                                128
                            } else {
                                u32::from(control.unsigned_abs())
                            };
                            self.take(usize::try_from(count).map_err(|_| Error::ResourceLimit)?)?;
                            count
                        } else {
                            self.byte()?;
                            u32::from((control >> 3).unsigned_abs())
                        };
                        decoded = decoded
                            .checked_sub(count)
                            .ok_or(Error::InlineSprite(start))?;
                    }
                }
                RecordKind::InlineSprite {
                    flags,
                    bytes: self
                        .bytes
                        .get(payload_start..self.pos)
                        .ok_or(Error::Truncated(payload_start))?,
                }
            }
        };
        Ok(Some(Record {
            span: start..self.pos,
            declared_length: length,
            kind,
        }))
    }
    pub(super) fn record_header(&mut self, version: u8) -> Result<Option<(u32, u8)>, Error> {
        let length = self.length(version)?;
        if length == 0 {
            return Ok(None);
        }
        Ok(Some((length, self.byte()?)))
    }
}

pub(super) fn header(bytes: &[u8]) -> Result<(Reader<'_>, u8, Option<usize>), Error> {
    let mut reader = Reader { bytes, pos: 0 };
    if reader.word()? != 0 {
        reader.pos = 0;
        return Ok((reader, 1, None));
    }
    if reader.take(8)? != b"GRF\x82\r\n\x1a\n" {
        return Err(Error::Signature);
    }
    let offset = usize::try_from(reader.dword()?).map_err(|_| Error::SectionOffset)?;
    let section = 14_usize.checked_add(offset).ok_or(Error::SectionOffset)?;
    let compression = reader.byte()?;
    if compression != 0 {
        return Err(Error::Compression(compression));
    }
    Ok((reader, 2, Some(section)))
}
