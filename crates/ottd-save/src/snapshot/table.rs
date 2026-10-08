use super::{FieldValue, SnapshotError, invalid};
use crate::{Chunk, ChunkKind, Reader};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Fields = BTreeMap<String, FieldValue>;

pub(super) fn decode(
    chunk: &Chunk,
    expected: &[(&str, u8)],
) -> Result<Vec<(u32, Fields)>, SnapshotError> {
    if chunk.kind() != ChunkKind::Table {
        return Err(invalid("typed chunks require ordinary table encoding"));
    }
    let mut input = Reader::new(chunk.body());
    let header_len = input
        .gamma()?
        .checked_sub(1)
        .ok_or_else(|| invalid("missing schema"))?;
    let mut header = Reader::new(input.take(header_len)?);
    let mut schema = Vec::new();
    let mut names = BTreeSet::new();
    loop {
        let kind = header.byte()?;
        if kind == 0 {
            break;
        }
        if schema.len() >= 512 {
            return Err(invalid("schema field limit"));
        }
        let name = string(&mut header)?;
        if !names.insert(name.clone()) {
            return Err(invalid(format!("duplicate field {name}")));
        }
        let Some((_, expected_kind)) = expected.iter().find(|(key, _)| *key == name) else {
            return Err(invalid(format!("unknown field {name}")));
        };
        if kind != *expected_kind {
            return Err(invalid(format!("wrong wire type for {name}")));
        }
        schema.push((name, kind));
    }
    if !header.remaining.is_empty() || schema.len() != expected.len() {
        return Err(invalid("incomplete or trailing schema"));
    }
    let mut records = Vec::new();
    let mut owner = 0_u32;
    loop {
        let length = input.gamma()?;
        let Some(length) = length.checked_sub(1) else {
            break;
        };
        if owner >= 256 {
            return Err(invalid("table record limit"));
        }
        let mut record = Reader::new(input.take(length)?);
        if length != 0 {
            let fields = schema
                .iter()
                .map(|(name, kind)| Ok((name.clone(), value(&mut record, *kind)?)))
                .collect::<Result<_, SnapshotError>>()?;
            if !record.remaining.is_empty() {
                return Err(invalid("trailing table record bytes"));
            }
            records.push((owner, fields));
        }
        owner = owner.saturating_add(1);
    }
    if !input.remaining.is_empty() {
        return Err(invalid("trailing table bytes"));
    }
    Ok(records)
}

pub(super) fn single(chunk: &Chunk, expected: &[(&str, u8)]) -> Result<Fields, SnapshotError> {
    let mut records = decode(chunk, expected)?;
    if records.len() != 1 {
        return Err(invalid("table requires exactly one record"));
    }
    let (index, fields) = records.pop().ok_or_else(|| invalid("missing record"))?;
    if index != 0 {
        return Err(invalid("singleton record must have index zero"));
    }
    Ok(fields)
}

fn string(input: &mut Reader<'_>) -> Result<String, SnapshotError> {
    let length = input.gamma()?;
    if length > 65_536 {
        return Err(invalid("string length limit"));
    }
    String::from_utf8(input.take(length)?.to_vec()).map_err(|_| invalid("invalid UTF-8 string"))
}

fn value(input: &mut Reader<'_>, kind: u8) -> Result<FieldValue, SnapshotError> {
    if kind == 26 {
        return Ok(FieldValue::String(string(input)?));
    }
    if kind & 16 != 0 {
        let length = input.gamma()?;
        if length > 65_536 || length > input.remaining.len() {
            return Err(invalid("array length limit"));
        }
        return (0..length)
            .map(|_| scalar(input, kind & 15))
            .collect::<Result<Vec<_>, _>>()
            .map(FieldValue::Array);
    }
    scalar(input, kind)
}

fn scalar(input: &mut Reader<'_>, kind: u8) -> Result<FieldValue, SnapshotError> {
    let width = match kind {
        1 | 2 => 1,
        3 | 4 => 2,
        5 | 6 => 4,
        7 | 8 => 8,
        _ => return Err(invalid("unsupported field wire type")),
    };
    let bytes = input.take(width)?;
    let unsigned = bytes
        .iter()
        .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte));
    if kind % 2 == 0 {
        return Ok(FieldValue::Unsigned(unsigned));
    }
    let mut signed = if bytes.first().is_some_and(|b| b & 128 != 0) {
        -1_i64
    } else {
        0
    };
    for byte in bytes {
        signed = (signed << 8) | i64::from(*byte);
    }
    Ok(FieldValue::Signed(signed))
}

pub(super) fn unsigned<T: TryFrom<u64>>(fields: &Fields, name: &str) -> Result<T, SnapshotError> {
    match fields.get(name) {
        Some(FieldValue::Unsigned(value)) => {
            T::try_from(*value).map_err(|_| invalid(format!("{name} out of range")))
        }
        _ => Err(invalid(format!("{name} is not unsigned"))),
    }
}

pub(super) fn validate_settings(fields: &mut Fields) -> Result<(), SnapshotError> {
    if fields.len() != super::schema::SETTINGS.len() {
        return Err(invalid("incomplete settings"));
    }
    for (name, kind) in super::schema::SETTINGS {
        let field = fields
            .get_mut(*name)
            .ok_or_else(|| invalid(format!("missing setting {name}")))?;
        // JSON integers carry no signedness. Restore it from the pinned descriptor.
        match (&*field, *kind) {
            (FieldValue::Signed(value), 2 | 4 | 6 | 8) if *value >= 0 => {
                *field = FieldValue::Unsigned(
                    u64::try_from(*value).map_err(|_| invalid("integer range"))?,
                );
            }
            (FieldValue::Unsigned(value), 1 | 3 | 5 | 7) => {
                *field = FieldValue::Signed(
                    i64::try_from(*value).map_err(|_| invalid("integer range"))?,
                );
            }
            _ => {}
        }
        let valid = match (&*field, *kind) {
            (FieldValue::Signed(value), 1) => (0..=1).contains(value),
            (FieldValue::Signed(value), 3) => i16::try_from(*value).is_ok(),
            (FieldValue::Signed(value), 5) => i32::try_from(*value).is_ok(),
            (FieldValue::Signed(_), 7) | (FieldValue::Unsigned(_), 8) => true,
            (FieldValue::Unsigned(value), 2) => u8::try_from(*value).is_ok(),
            (FieldValue::Unsigned(value), 4) => u16::try_from(*value).is_ok(),
            (FieldValue::Unsigned(value), 6) => u32::try_from(*value).is_ok(),
            (FieldValue::String(value), 26) => value.len() <= 65_536,
            _ => false,
        };
        if !valid {
            return Err(invalid(format!("invalid setting {name}")));
        }
    }
    Ok(())
}
