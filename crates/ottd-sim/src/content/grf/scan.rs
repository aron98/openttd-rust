use super::{
    action14,
    container::checksum_extent,
    metadata_types::ScanOptions,
    records::{Reader, header},
    text::{Budget, localized},
    types::{
        GrfIdentity, GrfParseError, Metadata, RecordKind, ScanError, ScanFailure, ScanOutcome,
        ScanStatus,
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

fn apply_action8<'a>(
    outcome: &mut ScanOutcome<'a>,
    metadata: Metadata<'a>,
    bytes: &[u8],
    budget: &mut Budget,
    offset: usize,
) -> Result<(), ScanError> {
    outcome.invalid_version = !(2..=8).contains(&metadata.version);
    outcome.static_info.name.insert(localized(
        metadata.name,
        127,
        metadata.grfid,
        false,
        budget,
        offset,
    )?);
    if let Some(raw) = metadata.info {
        outcome.static_info.description.insert(localized(
            raw,
            127,
            metadata.grfid,
            true,
            budget,
            offset,
        )?);
    }
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
    Ok(())
}

/// Scan until native FILESCAN stops; subsequent malformed bytes are not executed.
/// # Errors
/// Returns structural or host resource-limit errors.
pub fn scan_file(bytes: &[u8]) -> Result<ScanOutcome<'_>, ScanError> {
    scan_file_with_options(bytes, ScanOptions::default())
}

/// Fresh-registry FILESCAN with explicit client settings and host bounds.
/// # Errors
/// Returns structural or host resource-limit errors.
pub fn scan_file_with_options(
    bytes: &[u8],
    options: ScanOptions,
) -> Result<ScanOutcome<'_>, ScanError> {
    let limits = options.limits;
    let mut budget = Budget::new(limits);
    if bytes.len() > limits.bytes {
        return Err(ScanError::ResourceLimit {
            resource: "bytes",
            offset: 0,
        });
    }
    let (mut reader, version, _) = header(bytes)?;
    reader.initial(version)?;
    let mut outcome = ScanOutcome::fresh(options.language);
    let mut skip_count = 0_u32;
    let mut count = 0_usize;
    loop {
        let record_start = reader.pos;
        let Some((length, kind)) = reader.record_header(version)? else {
            break;
        };
        count = count.saturating_add(1);
        if count > limits.records {
            return Err(ScanError::ResourceLimit {
                resource: "records",
                offset: record_start,
            });
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
        let payload_start = reader.pos.saturating_sub(payload.len());
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
            0x14 => {
                match action14::parse(
                    &mut action_reader,
                    &mut outcome.static_info,
                    &mut budget,
                    payload_start,
                ) {
                    Ok(()) => (),
                    Err(ScanError::Structure(GrfParseError::Truncated(_))) => {
                        outcome.status = ScanStatus::Disabled;
                        outcome.failure = Some((ScanFailure::ReadBounds, line));
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
            8 => {
                let Ok(metadata) = metadata(&mut action_reader) else {
                    outcome.status = ScanStatus::Disabled;
                    outcome.failure = Some((ScanFailure::ReadBounds, line));
                    break;
                };
                apply_action8(&mut outcome, metadata, bytes, &mut budget, record_start)?;
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
    outcome.static_info.finalize(options.default_palette);
    Ok(outcome)
}
