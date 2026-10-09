use super::{
    container::checksum_extent,
    records::{Reader, header},
    types::{
        GrfIdentity, GrfParseError, Metadata, ParseLimits, RecordKind, ScanError, ScanFailure,
        ScanOutcome, ScanStatus,
    },
};
use md5::{Digest, Md5};

fn skip(action: u8, reader: &mut Reader<'_>) -> Result<u32, GrfParseError> {
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

fn metadata<'a>(reader: &mut Reader<'a>) -> Result<Metadata<'a>, GrfParseError> {
    Ok(Metadata {
        version: reader.byte()?,
        grfid: reader.dword()?,
        name: reader.string()?,
        info: if reader.remaining() == 0 {
            None
        } else {
            Some(reader.string()?)
        },
    })
}

/// Scan until native FILESCAN stops; subsequent malformed bytes are not executed.
/// Action14 remains explicitly unsupported until its metadata interpreter is added.
/// # Errors
/// Returns unsafe framing, resource-limit or unsupported metadata-action errors.
pub fn scan_file(bytes: &[u8]) -> Result<ScanOutcome<'_>, ScanError> {
    let limits = ParseLimits::default();
    if bytes.len() > limits.bytes {
        return Err(GrfParseError::ResourceLimit.into());
    }
    let (mut reader, version, _) = header(bytes)?;
    reader.initial(version)?;
    let mut outcome = ScanOutcome {
        failure: None,
        status: ScanStatus::Unknown,
        accepted: false,
        identity: None,
        metadata: None,
        invalid_version: false,
        system: false,
    };
    let mut skip_count = 0_u32;
    let mut count = 0_usize;
    loop {
        let record_start = reader.pos;
        let Some((length, kind)) = reader.record_header(version)? else {
            break;
        };
        count = count.saturating_add(1);
        if count > limits.records {
            return Err(GrfParseError::ResourceLimit.into());
        }
        let line = u32::try_from(count).map_err(|_| GrfParseError::ResourceLimit)?;
        if skip_count == 0 && (kind != 255 || length > 1024 * 1024) {
            outcome.status = ScanStatus::Disabled;
            outcome.failure = Some((ScanFailure::UnexpectedSprite, line));
            break;
        }
        reader.pos = record_start;
        let Some(record) = reader.record(version)? else {
            break;
        };
        if skip_count > 0 {
            skip_count = skip_count.saturating_sub(1);
            continue;
        }
        let RecordKind::Pseudo(payload) = record.kind else {
            return Err(GrfParseError::InlineSprite(record_start).into());
        };
        let mut action_reader = Reader {
            bytes: payload,
            pos: 0,
        };
        let Ok(action) = action_reader.byte() else {
            outcome.status = ScanStatus::Disabled;
            outcome.failure = Some((ScanFailure::ReadBounds, line));
            break;
        };
        match action {
            0x14 => return Err(ScanError::UnsupportedAction(action)),
            8 => {
                let Ok(metadata) = metadata(&mut action_reader) else {
                    outcome.status = ScanStatus::Disabled;
                    outcome.failure = Some((ScanFailure::ReadBounds, line));
                    break;
                };
                outcome.invalid_version = !(2..=8).contains(&metadata.version);
                outcome.system = metadata.grfid & 255 == 255;
                outcome.accepted = metadata.grfid != 0 && !outcome.system;
                if outcome.accepted {
                    let data = bytes
                        .get(..checksum_extent(bytes))
                        .ok_or(GrfParseError::SectionOffset)?;
                    outcome.identity = Some(GrfIdentity {
                        grfid: metadata.grfid,
                        md5: Md5::digest(data).into(),
                    });
                }
                outcome.metadata = Some(metadata);
                break;
            }
            _ => {
                if let Ok(value) = skip(action, &mut action_reader) {
                    skip_count = value;
                } else {
                    outcome.status = ScanStatus::Disabled;
                    outcome.failure = Some((ScanFailure::ReadBounds, line));
                    break;
                }
            }
        }
    }
    Ok(outcome)
}
