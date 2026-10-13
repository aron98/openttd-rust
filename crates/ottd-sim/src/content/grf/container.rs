use super::{
    records::{Reader, header},
    types::{GrfParseError as Error, ParseLimits, Record, SpriteVariant},
};

/// Complete structural inspection retaining encoded sprites and source order.
#[derive(Debug, Clone)]
pub struct GrfContainer<'a> {
    /// Native container version (1 or 2).
    pub version: u8,
    /// Ordered data records after the ignored initial sprite-count record.
    pub records: Vec<Record<'a>>,
    /// Ordered v2 sprite entries including variants and duplicate IDs.
    pub sprites: Vec<SpriteVariant<'a>>,
    /// Native identity checksum extent, possibly shorter than the file.
    pub checksum_extent: usize,
}
impl<'a> GrfContainer<'a> {
    /// Parse with bounded default resource policy; this does not activate content.
    /// # Errors
    /// Returns malformed framing or host resource-limit errors.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        Self::parse_with_limits(bytes, ParseLimits::default())
    }
    /// Parse with explicit host limits, separate from native GRF status.
    /// # Errors
    /// Returns malformed framing or caller-selected resource-limit errors.
    pub fn parse_with_limits(bytes: &'a [u8], limits: ParseLimits) -> Result<Self, Error> {
        if bytes.len() > limits.bytes {
            return Err(Error::ResourceLimit);
        }
        let (mut reader, version, section) = header(bytes)?;
        if let Some(end) = section {
            if end < reader.pos || end > bytes.len() {
                return Err(Error::SectionOffset);
            }
            reader.bytes = bytes.get(..end).ok_or(Error::SectionOffset)?;
        }
        reader.initial(version)?;
        let mut records = Vec::new();
        while let Some(record) = reader.record(version)? {
            if records.len() >= limits.records {
                return Err(Error::ResourceLimit);
            }
            records.push(record);
        }
        let mut sprites = Vec::new();
        if let Some(start) = section {
            let mut reader = Reader { bytes, pos: start };
            loop {
                let start = reader.pos;
                let id = reader.dword()?;
                if id == 0 {
                    break;
                }
                if records.len().saturating_add(sprites.len()) >= limits.records {
                    return Err(Error::ResourceLimit);
                }
                let length = usize::try_from(reader.dword()?).map_err(|_| Error::ResourceLimit)?;
                let payload = reader.take(length)?;
                sprites.push(SpriteVariant {
                    id,
                    span: start..reader.pos,
                    bytes: payload,
                });
            }
        }
        Ok(Self {
            version,
            records,
            sprites,
            checksum_extent: checksum_extent(bytes),
        })
    }
}

pub(super) fn checksum_extent(bytes: &[u8]) -> usize {
    if bytes.get(..10) != Some(b"\0\0GRF\x82\r\n\x1a\n") {
        return bytes.len();
    }
    let Some(offset) = bytes
        .get(10..14)
        .and_then(|v| <[u8; 4]>::try_from(v).ok())
        .map(u32::from_le_bytes)
    else {
        return bytes.len();
    };
    if offset >= 1 << 30 {
        return bytes.len();
    }
    usize::try_from(offset)
        .ok()
        .and_then(|v| v.checked_add(14))
        .unwrap_or(bytes.len())
        .min(bytes.len())
}
