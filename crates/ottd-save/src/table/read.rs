use super::{
    Budget, FieldSchema, TableChunk, TableError, TableLimits, TableRecord, TableSchema,
    TableTailPolicy, WireValue, allow_tail, invalid,
};
use crate::{Chunk, ChunkKind, Reader};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn decode(
    chunk: &Chunk,
    policy: TableTailPolicy,
    limits: TableLimits,
) -> Result<TableChunk, TableError> {
    let sparse = match chunk.kind() {
        ChunkKind::Table => false,
        ChunkKind::SparseTable => true,
        ChunkKind::Riff | ChunkKind::Array | ChunkKind::SparseArray => {
            return Err(invalid("chunk", "requires table encoding"));
        }
    };
    let tails = allow_tail(chunk.id(), policy)?;
    if chunk.body().len() > limits.max_bytes {
        return Err(TableError::Limit("wire bytes"));
    }
    let mut budget = Budget::new(limits);
    let mut input = Reader::new(chunk.body());
    let length = input
        .gamma()?
        .checked_sub(1)
        .ok_or_else(|| invalid("schema", "missing header"))?;
    let mut header = Reader::new(input.take(length)?);
    let schema = schema(&mut header, &mut budget, 1)?;
    if !header.remaining.is_empty() {
        return Err(invalid("schema", "trailing header bytes"));
    }
    let mut records = BTreeMap::new();
    let mut slots = 0_u32;
    loop {
        let length = input.gamma()?;
        let Some(length) = length.checked_sub(1) else {
            break;
        };
        budget.items::<(u32, TableRecord)>(1)?;
        let mut row = Reader::new(input.take(length)?);
        let id = if sparse {
            u32::try_from(row.gamma()?).map_err(|_| invalid("record", "index overflow"))?
        } else {
            slots
        };
        slots = slots
            .checked_add(1)
            .ok_or_else(|| invalid("record", "too many indices"))?;
        if !sparse && length == 0 {
            continue;
        }
        let path = format!("record[{id}]");
        let mut record = record(&mut row, &schema, &mut budget, 1, &path)?;
        if tails {
            budget.charge(0, row.remaining.len())?;
            record.tail = row.remaining.to_vec();
        } else if !row.remaining.is_empty() {
            return Err(invalid(&path, "trailing record bytes"));
        }
        if records.insert(id, record).is_some() {
            return Err(invalid(&path, "duplicate record index"));
        }
    }
    if !input.remaining.is_empty() {
        return Err(invalid("chunk", "trailing table bytes"));
    }
    Ok(TableChunk {
        id: chunk.id(),
        kind: chunk.kind(),
        schema,
        records,
        slots,
        tail_policy: policy,
        limits,
    })
}

fn schema(
    input: &mut Reader<'_>,
    budget: &mut Budget,
    depth: usize,
) -> Result<TableSchema, TableError> {
    budget.depth(depth)?;
    let mut fields = Vec::new();
    let mut names = BTreeSet::new();
    loop {
        let kind = input.byte()?;
        if kind == 0 {
            break;
        }
        if !matches!(kind,1..=9|17..=27) {
            return Err(invalid("schema", "unsupported wire type"));
        }
        budget.items::<FieldSchema>(1)?;
        let length = input.gamma()?;
        // Names also occur in the duplicate-detection set.
        budget.charge(
            0,
            length
                .checked_mul(2)
                .ok_or(TableError::Limit("allocation bytes"))?,
        )?;
        let name = String::from_utf8(input.take(length)?.to_vec())
            .map_err(|_| invalid("schema", "non-UTF-8 field name"))?;
        if name.is_empty() || !names.insert(name.clone()) {
            return Err(invalid("schema", "empty or duplicate field name"));
        }
        fields.push(FieldSchema {
            name,
            wire_type: kind,
            child: None,
        });
    }
    for field in &mut fields {
        if field.wire_type == 27 {
            field.child = Some(schema(input, budget, depth.saturating_add(1))?);
        }
    }
    Ok(TableSchema { fields })
}

fn record(
    input: &mut Reader<'_>,
    schema: &TableSchema,
    budget: &mut Budget,
    depth: usize,
    path: &str,
) -> Result<TableRecord, TableError> {
    budget.depth(depth)?;
    budget.items::<WireValue>(schema.fields.len())?;
    let mut values = Vec::new();
    for field in &schema.fields {
        let path = format!("{path}.{}", field.name);
        values.push(value(input, field, budget, depth, &path)?);
    }
    Ok(TableRecord {
        values,
        tail: Vec::new(),
    })
}
fn value(
    input: &mut Reader<'_>,
    field: &FieldSchema,
    budget: &mut Budget,
    depth: usize,
    path: &str,
) -> Result<WireValue, TableError> {
    match field.wire_type {
        26 => {
            let length = input.gamma()?;
            budget.charge(0, length)?;
            Ok(WireValue::Bytes(input.take(length)?.to_vec()))
        }
        27 => {
            let count = input.gamma()?;
            budget.items::<TableRecord>(count)?;
            let child = field
                .child
                .as_ref()
                .ok_or_else(|| invalid(path, "missing child schema"))?;
            let mut records = Vec::new();
            for index in 0..count {
                records.push(record(
                    input,
                    child,
                    budget,
                    depth.saturating_add(1),
                    &format!("{path}[{index}]"),
                )?);
            }
            Ok(WireValue::Structs(records))
        }
        17..=25 => {
            let count = input.gamma()?;
            budget.items::<WireValue>(count)?;
            let mut values = Vec::new();
            for _ in 0..count {
                values.push(scalar(input, field.wire_type & 15)?);
            }
            Ok(WireValue::Array(values))
        }
        1..=9 => scalar(input, field.wire_type),
        _ => Err(invalid(path, "unsupported wire type")),
    }
}
fn scalar(input: &mut Reader<'_>, kind: u8) -> Result<WireValue, TableError> {
    let width = match kind {
        1 | 2 => 1,
        3 | 4 | 9 => 2,
        5 | 6 => 4,
        7 | 8 => 8,
        _ => return Err(invalid("value", "unsupported scalar type")),
    };
    let bytes = input.take(width)?;
    if matches!(kind, 1 | 3 | 5 | 7) {
        let mut value = if bytes.first().is_some_and(|b| b & 128 != 0) {
            -1_i64
        } else {
            0
        };
        for byte in bytes {
            value = (value << 8) | i64::from(*byte);
        }
        Ok(WireValue::Signed(value))
    } else {
        Ok(WireValue::Unsigned(
            bytes.iter().fold(0_u64, |v, b| (v << 8) | u64::from(*b)),
        ))
    }
}
