use super::{Row, TableChunk, TableRecord, WireValue, WorldError, invalid};
use std::collections::BTreeMap;

pub(super) fn validate(tables: &BTreeMap<[u8; 4], TableChunk>) -> Result<(), WorldError> {
    for id in [*b"AIPL", *b"GSDT"] {
        let table = tables
            .get(&id)
            .ok_or_else(|| invalid("scripts", "missing script table"))?;
        let expected = if id == *b"AIPL" { 15 } else { 1 };
        if table.records().len() != expected
            || table
                .records()
                .keys()
                .copied()
                .ne(0..u32::try_from(expected)
                    .map_err(|_| invalid("scripts", "invalid owner count"))?)
        {
            return Err(invalid(
                "scripts",
                "missing or invalid script owner records",
            ));
        }
        for (owner, record) in table.records() {
            if id == *b"AIPL" {
                let companies = tables
                    .get(b"PLYR")
                    .ok_or_else(|| invalid("AIPL", "missing company pool"))?;
                let active = match companies.records().get(owner) {
                    Some(record) => matches!(
                        Row {
                            schema: companies.schema(),
                            record
                        }
                        .value("is_ai")?,
                        WireValue::Signed(1) | WireValue::Unsigned(1)
                    ),
                    None => false,
                };
                if active == record.tail().is_empty() {
                    return Err(invalid(
                        "AIPL",
                        "running script tail disagrees with company AI state",
                    ));
                }
            }
            let (_, data) = parts(id, record.tail())?;
            if data.is_empty() {
                if id == *b"GSDT" || !record.tail().is_empty() {
                    return Err(invalid("GSDT", "missing script presence byte"));
                }
                continue;
            }
            let mut reader = Reader { bytes: data };
            match reader.byte()? {
                0 => {}
                1 => reader.object(0)?,
                _ => return Err(invalid("script_data", "invalid presence flag")),
            }
            if !reader.bytes.is_empty() {
                return Err(invalid("script_data", "trailing serialized data"));
            }
        }
    }
    Ok(())
}
pub(super) fn export(
    id: [u8; 4],
    record: &TableRecord,
    fields: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), WorldError> {
    if !matches!(&id, b"AIPL" | b"GSDT") {
        return Ok(());
    }
    let (running, data) = parts(id, record.tail())?;
    if let Some((name, settings, version)) = running {
        fields.insert("running_name".into(), serde_json::json!(name));
        fields.insert("running_settings".into(), serde_json::json!(settings));
        fields.insert("running_version".into(), serde_json::json!(version));
    }
    if !data.is_empty() {
        fields.insert("script_data".into(), serde_json::json!(data));
    }
    Ok(())
}
type Running<'a> = Option<(&'a [u8], &'a [u8], u32)>;
fn parts(id: [u8; 4], tail: &[u8]) -> Result<(Running<'_>, &[u8]), WorldError> {
    if id != *b"AIPL" || tail.is_empty() {
        return Ok((None, tail));
    }
    let mut reader = Reader { bytes: tail };
    let name_len = reader.gamma()?;
    let name = reader.take(name_len)?;
    let settings_len = reader.gamma()?;
    let settings = reader.take(settings_len)?;
    let version = u32::from_be_bytes(
        reader
            .take(4)?
            .try_into()
            .map_err(|_| invalid("AIPL", "invalid version"))?,
    );
    Ok((Some((name, settings, version)), reader.bytes))
}
struct Reader<'a> {
    bytes: &'a [u8],
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], WorldError> {
        let (head, rest) = self
            .bytes
            .split_at_checked(n)
            .ok_or_else(|| invalid("script_data", "truncated serialized data"))?;
        self.bytes = rest;
        Ok(head)
    }
    fn byte(&mut self) -> Result<u8, WorldError> {
        self.take(1)?
            .first()
            .copied()
            .ok_or_else(|| invalid("script_data", "truncated byte"))
    }
    fn gamma(&mut self) -> Result<usize, WorldError> {
        let first = self.byte()?;
        let (mut value, extra) = match first {
            0..=127 => (usize::from(first), 0),
            128..=191 => (usize::from(first & 63), 1),
            192..=223 => (usize::from(first & 31), 2),
            224..=239 => (usize::from(first & 15), 3),
            240 => (0, 4),
            _ => return Err(invalid("script_data", "invalid string length")),
        };
        for _ in 0..extra {
            value = (value << 8) | usize::from(self.byte()?);
        }
        Ok(value)
    }
    fn object(&mut self, depth: u32) -> Result<(), WorldError> {
        if depth >= 25 {
            return Err(invalid("script_data", "script depth limit"));
        }
        let tag = self.byte()?;
        match tag {
            0 => {
                self.take(8)?;
            }
            1 => {
                let length = usize::from(self.byte()?);
                let bytes = self.take(length)?;
                if bytes.last() != Some(&0) {
                    return Err(invalid("script_data", "string lacks terminator"));
                }
            }
            2 | 3 => {
                let mut count = 0usize;
                loop {
                    if self.bytes.first() == Some(&255) {
                        self.byte()?;
                        break;
                    }
                    self.object(depth.saturating_add(1))?;
                    count = count.saturating_add(1);
                }
                if tag == 3 && count % 2 != 0 {
                    return Err(invalid("script_data", "table has unmatched key"));
                }
            }
            4 => {
                if self.byte()? > 1 {
                    return Err(invalid("script_data", "invalid boolean"));
                }
            }
            5 => {}
            6 => {
                if self.bytes.first() != Some(&1) {
                    return Err(invalid("script_data", "instance class is not a string"));
                }
                self.object(depth.saturating_add(1))?;
                self.object(depth.saturating_add(1))?;
            }
            _ => return Err(invalid("script_data", "unknown script object type")),
        }
        Ok(())
    }
}
