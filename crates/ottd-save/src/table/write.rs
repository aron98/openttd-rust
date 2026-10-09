use super::{
    Budget, FieldSchema, TableChunk, TableError, TableRecord, TableSchema, WireValue, allow_tail,
    invalid,
};
use crate::{Chunk, ChunkKind};

pub(super) fn encode(table: &TableChunk) -> Result<Chunk, TableError> {
    let tails = allow_tail(table.id, table.tail_policy)?;
    let mut budget = Budget::new(table.limits);
    let mut output = Output::new(table.limits.max_bytes);
    let mut header = Output::new(table.limits.max_bytes);
    schema(&table.schema, &mut header, &mut budget, 1)?;
    output.frame(&header.bytes)?;
    match table.kind {
        ChunkKind::Table => {
            let last = table.records.last_key_value().map_or(Ok(0), |(id, _)| {
                id.checked_add(1)
                    .ok_or_else(|| invalid("record", "index overflow"))
            })?;
            let count = table.slots.max(last);
            budget.items::<(u32, TableRecord)>(
                usize::try_from(count).map_err(|_| TableError::Limit("elements"))?,
            )?;
            for index in 0..count {
                match table.records.get(&index) {
                    Some(row) => {
                        let mut encoded = Output::new(table.limits.max_bytes);
                        record(
                            row,
                            &table.schema,
                            &mut encoded,
                            &mut budget,
                            1,
                            &format!("record[{index}]"),
                        )?;
                        tail(row, tails, &mut encoded)?;
                        if encoded.bytes.is_empty() {
                            return Err(invalid(
                                "record",
                                "empty ordinary record is indistinguishable from a hole",
                            ));
                        }
                        output.frame(&encoded.bytes)?;
                    }
                    None => output.gamma(1)?,
                }
            }
        }
        ChunkKind::SparseTable => {
            budget.items::<(u32, TableRecord)>(table.records.len())?;
            for (index, row) in &table.records {
                let mut encoded = Output::new(table.limits.max_bytes);
                encoded.gamma(
                    usize::try_from(*index).map_err(|_| invalid("record", "index overflow"))?,
                )?;
                record(
                    row,
                    &table.schema,
                    &mut encoded,
                    &mut budget,
                    1,
                    &format!("record[{index}]"),
                )?;
                tail(row, tails, &mut encoded)?;
                output.frame(&encoded.bytes)?;
            }
        }
        ChunkKind::Riff | ChunkKind::Array | ChunkKind::SparseArray => {
            return Err(invalid("chunk", "requires table encoding"));
        }
    }
    output.byte(0)?;
    Ok(Chunk::from_table(table.id, table.kind, output.bytes)?)
}
fn tail(row: &TableRecord, allowed: bool, output: &mut Output) -> Result<(), TableError> {
    if !allowed && !row.tail.is_empty() {
        return Err(invalid("record", "disallowed tail"));
    }
    output.extend(&row.tail)
}
fn schema(
    schema: &TableSchema,
    output: &mut Output,
    budget: &mut Budget,
    depth: usize,
) -> Result<(), TableError> {
    budget.depth(depth)?;
    budget.items::<FieldSchema>(schema.fields.len())?;
    for field in &schema.fields {
        output.byte(field.wire_type)?;
        output.gamma(field.name.len())?;
        output.extend(field.name.as_bytes())?;
    }
    output.byte(0)?;
    for field in &schema.fields {
        if let Some(child) = &field.child {
            self::schema(child, output, budget, depth.saturating_add(1))?;
        }
    }
    Ok(())
}
fn record(
    row: &TableRecord,
    schema: &TableSchema,
    output: &mut Output,
    budget: &mut Budget,
    depth: usize,
    path: &str,
) -> Result<(), TableError> {
    budget.depth(depth)?;
    if row.values.len() != schema.fields.len() {
        return Err(invalid(path, "record field count mismatch"));
    }
    budget.items::<WireValue>(row.values.len())?;
    for (field, value) in schema.fields.iter().zip(&row.values) {
        value_write(
            value,
            field,
            output,
            budget,
            depth,
            &format!("{path}.{}", field.name),
        )?;
    }
    Ok(())
}
fn value_write(
    value: &WireValue,
    field: &FieldSchema,
    output: &mut Output,
    budget: &mut Budget,
    depth: usize,
    path: &str,
) -> Result<(), TableError> {
    match value {
        WireValue::Signed(_) | WireValue::Unsigned(_) => {
            scalar(value, field.wire_type, output, path)
        }
        WireValue::Bytes(bytes) => {
            if field.wire_type != 26 {
                return Err(invalid(path, "expected string descriptor"));
            }
            budget.charge(0, bytes.len())?;
            output.gamma(bytes.len())?;
            output.extend(bytes)
        }
        WireValue::Array(values) => {
            if !matches!(field.wire_type, 17..=25) {
                return Err(invalid(path, "expected primitive array descriptor"));
            }
            budget.items::<WireValue>(values.len())?;
            output.gamma(values.len())?;
            for value in values {
                scalar(value, field.wire_type & 15, output, path)?;
            }
            Ok(())
        }
        WireValue::Structs(rows) => {
            if field.wire_type != 27 {
                return Err(invalid(path, "expected struct descriptor"));
            }
            let child = field
                .child
                .as_ref()
                .ok_or_else(|| invalid(path, "missing child schema"))?;
            budget.items::<TableRecord>(rows.len())?;
            output.gamma(rows.len())?;
            for (index, row) in rows.iter().enumerate() {
                record(
                    row,
                    child,
                    output,
                    budget,
                    depth.saturating_add(1),
                    &format!("{path}[{index}]"),
                )?;
                tail(row, false, output)?;
            }
            Ok(())
        }
    }
}
fn scalar(value: &WireValue, kind: u8, output: &mut Output, path: &str) -> Result<(), TableError> {
    let range = || invalid(path, "integer width or signedness mismatch");
    match (kind, value) {
        (1, WireValue::Signed(v)) => {
            output.extend(&i8::try_from(*v).map_err(|_| range())?.to_be_bytes())
        }
        (2, WireValue::Unsigned(v)) => {
            output.extend(&u8::try_from(*v).map_err(|_| range())?.to_be_bytes())
        }
        (3, WireValue::Signed(v)) => {
            output.extend(&i16::try_from(*v).map_err(|_| range())?.to_be_bytes())
        }
        (4 | 9, WireValue::Unsigned(v)) => {
            output.extend(&u16::try_from(*v).map_err(|_| range())?.to_be_bytes())
        }
        (5, WireValue::Signed(v)) => {
            output.extend(&i32::try_from(*v).map_err(|_| range())?.to_be_bytes())
        }
        (6, WireValue::Unsigned(v)) => {
            output.extend(&u32::try_from(*v).map_err(|_| range())?.to_be_bytes())
        }
        (7, WireValue::Signed(v)) => output.extend(&v.to_be_bytes()),
        (8, WireValue::Unsigned(v)) => output.extend(&v.to_be_bytes()),
        _ => Err(range()),
    }
}
struct Output {
    bytes: Vec<u8>,
    limit: usize,
}
impl Output {
    const fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    fn extend(&mut self, bytes: &[u8]) -> Result<(), TableError> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(TableError::Limit("wire bytes"))?;
        if length > self.limit {
            return Err(TableError::Limit("wire bytes"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| TableError::Limit("allocation bytes"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), TableError> {
        self.extend(&[byte])
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<(), TableError> {
        self.gamma(
            bytes
                .len()
                .checked_add(1)
                .ok_or(TableError::Limit("wire bytes"))?,
        )?;
        self.extend(bytes)
    }
    fn gamma(&mut self, value: usize) -> Result<(), TableError> {
        let value = u32::try_from(value).map_err(|_| TableError::Limit("gamma length"))?;
        let [a, b, c, d] = value.to_be_bytes();
        match value {
            0..=127 => self.byte(d),
            128..=16_383 => self.extend(&[c | 0x80, d]),
            16_384..=2_097_151 => self.extend(&[b | 0xc0, c, d]),
            2_097_152..=268_435_455 => self.extend(&[a | 0xe0, b, c, d]),
            _ => self.extend(&[240, a, b, c, d]),
        }
    }
}
