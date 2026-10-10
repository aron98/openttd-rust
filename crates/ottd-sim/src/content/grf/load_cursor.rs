use super::{
    GrfParseError,
    records::{Reader, header},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Skip {
    None,
    Count(i32),
    Stop,
}
impl Skip {
    pub(super) const fn from_count(count: u32) -> Self {
        match i32::from_ne_bytes(count.to_ne_bytes()) {
            0 => Self::None,
            -1 => Self::Stop,
            count => Self::Count(count),
        }
    }
    pub(super) const fn value(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Stop => -1,
            Self::Count(count) => count,
        }
    }
}
pub(super) struct Cursor<'a> {
    pub reader: Reader<'a>,
    pub version: u8,
    pub line: u32,
    pub skip: Skip,
}
impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Result<Self, GrfParseError> {
        let (mut reader, version, _) = header(bytes)?;
        reader.initial(version)?;
        Ok(Self {
            reader,
            version,
            line: 0,
            skip: Skip::None,
        })
    }
    pub(super) const fn skipped(&mut self) {
        if let Skip::Count(count) = self.skip {
            self.skip = if count == 1 {
                Skip::None
            } else if count <= 0 {
                self.skip
            } else {
                Skip::Count(count.saturating_sub(1))
            };
        }
    }
}

pub(super) fn count_records(action: u8, reader: &mut Reader<'_>) -> Result<u32, GrfParseError> {
    match action {
        1 => {
            reader.byte()?;
            let mut sets = u32::from(reader.byte()?);
            if sets == 0 && reader.remaining() >= 3 {
                reader.extended()?;
                sets = u32::from(reader.extended()?);
            }
            Ok(sets.saturating_mul(u32::from(reader.extended()?)))
        }
        5 => {
            reader.byte()?;
            Ok(u32::from(reader.extended()?))
        }
        0x11 => Ok(u32::from(reader.word()?)),
        0x0a | 0x12 => {
            let mut count = 0_u32;
            for _ in 0..reader.byte()? {
                if action == 0x12 {
                    reader.byte()?;
                }
                count = count.saturating_add(u32::from(reader.byte()?));
                reader.word()?;
            }
            Ok(count)
        }
        _ => Ok(0),
    }
}
