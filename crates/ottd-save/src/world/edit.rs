use super::{
    PathElement, TableRecord, TableSchema, TileState, WireValue, World, WorldEdit, WorldError,
    invalid, name,
};

impl World {
    /// Apply coupled field and tile changes, validating only the final world.
    ///
    /// # Errors
    /// On failure all saved state and derived indexes remain unchanged.
    pub fn edit_batch(&mut self, edits: Vec<WorldEdit>) -> Result<(), WorldError> {
        let mut candidate = self.clone();
        for edit in edits {
            match edit {
                WorldEdit::InsertRecord {
                    chunk,
                    record,
                    value,
                } => {
                    let records = candidate.pool_records_mut(chunk)?;
                    match records.entry(record) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(value);
                        }
                        std::collections::btree_map::Entry::Occupied(_) => {
                            return Err(invalid(&name(chunk), "record already exists"));
                        }
                    }
                }
                WorldEdit::RemoveRecord { chunk, record } => {
                    if candidate.pool_records_mut(chunk)?.remove(&record).is_none() {
                        return Err(invalid(&name(chunk), "missing record"));
                    }
                }
                WorldEdit::ReplaceRecord {
                    chunk,
                    record,
                    value,
                } => {
                    let target = candidate
                        .pool_records_mut(chunk)?
                        .get_mut(&record)
                        .ok_or_else(|| invalid(&name(chunk), "missing record"))?;
                    *target = value;
                }
                WorldEdit::StructList {
                    chunk,
                    record,
                    path,
                    rows,
                } => {
                    let table = candidate
                        .tables
                        .get_mut(&chunk)
                        .ok_or_else(|| invalid(&name(chunk), "missing table"))?;
                    let schema = table.schema().clone();
                    let row = table
                        .records_mut()
                        .get_mut(&record)
                        .ok_or_else(|| invalid(&name(chunk), "missing record"))?;
                    let target = select_mut(&schema, row, &path)?;
                    if !matches!(target, WireValue::Structs(_)) {
                        return Err(invalid("edit", "expected structure list"));
                    }
                    *target = WireValue::Structs(rows);
                }
                WorldEdit::Field {
                    chunk,
                    record,
                    path,
                    value,
                } => candidate.stage_field(chunk, record, &path, value)?,
                WorldEdit::Tile { index, value } => candidate.stage_tile(index, &value)?,
            }
        }
        let replacement = Self::decode(&candidate.to_savegame()?)?;
        *self = replacement;
        Ok(())
    }

    /// Replace one existing saved field transactionally and rebuild indexes.
    /// # Errors
    /// Rejects invalid paths, wire values or resulting object relationships.
    pub fn edit_field(
        &mut self,
        chunk: [u8; 4],
        record: u32,
        path: &[PathElement],
        value: WireValue,
    ) -> Result<(), WorldError> {
        self.edit_batch(vec![WorldEdit::Field {
            chunk,
            record,
            path: path.to_vec(),
            value,
        }])
    }

    /// Replace one complete saved tile transactionally.
    /// # Errors
    /// Rejects out-of-map indices and invalid pool associations.
    pub fn edit_tile(&mut self, index: u32, tile: &TileState) -> Result<(), WorldError> {
        self.edit_batch(vec![WorldEdit::Tile {
            index,
            value: tile.clone(),
        }])
    }

    fn stage_field(
        &mut self,
        chunk: [u8; 4],
        record: u32,
        path: &[PathElement],
        value: WireValue,
    ) -> Result<(), WorldError> {
        let table = self
            .tables
            .get_mut(&chunk)
            .ok_or_else(|| invalid(&name(chunk), "missing table"))?;
        let schema = table.schema().clone();
        let row = table
            .records_mut()
            .get_mut(&record)
            .ok_or_else(|| invalid(&name(chunk), "missing record"))?;
        let target = select_mut(&schema, row, path)?;
        match (&*target, &value) {
            (WireValue::Array(old), WireValue::Array(new)) if old.len() != new.len() => {
                return Err(invalid(
                    "edit",
                    "list cardinality changes require an object operation",
                ));
            }
            (WireValue::Structs(_), _) | (_, WireValue::Structs(_)) => {
                return Err(invalid(
                    "edit",
                    "edit nested fields instead of replacing structures",
                ));
            }
            _ => {}
        }
        *target = value;
        Ok(())
    }

    fn pool_records_mut(
        &mut self,
        chunk: [u8; 4],
    ) -> Result<&mut std::collections::BTreeMap<u32, TableRecord>, WorldError> {
        if !matches!(
            &chunk,
            b"VEHS"
                | b"PLYR"
                | b"CITY"
                | b"INDY"
                | b"STNN"
                | b"ORDL"
                | b"BKOR"
                | b"CAPA"
                | b"CAPY"
                | b"DEPT"
                | b"ROAD"
                | b"OBJS"
                | b"ERNW"
                | b"ENGN"
                | b"GRPS"
                | b"PSAC"
                | b"SIGN"
                | b"SUBS"
                | b"GOAL"
                | b"STPA"
                | b"STPE"
                | b"LEAT"
                | b"LEAE"
                | b"LGRP"
                | b"LGRJ"
        ) {
            return Err(invalid(
                &name(chunk),
                "record operation requires a native pool",
            ));
        }
        self.tables
            .get_mut(&chunk)
            .map(super::TableChunk::records_mut)
            .ok_or_else(|| invalid(&name(chunk), "missing table"))
    }

    fn stage_tile(&mut self, index: u32, tile: &TileState) -> Result<(), WorldError> {
        let index = usize::try_from(index).map_err(|_| invalid("tile", "index out of range"))?;
        if index >= self.map().tiles().len() {
            return Err(invalid("tile", "index out of range"));
        }
        for (id, bytes) in [
            (*b"MAPT", vec![tile.tile_type()]),
            (*b"MAPH", vec![tile.height()]),
            (*b"MAPO", vec![tile.m1()]),
            (*b"MAP2", tile.m2().to_be_bytes().to_vec()),
            (*b"M3LO", vec![tile.m3()]),
            (*b"M3HI", vec![tile.m4()]),
            (*b"MAP5", vec![tile.m5()]),
            (*b"MAPE", vec![tile.m6()]),
            (*b"MAP7", vec![tile.m7()]),
            (*b"MAP8", tile.m8().to_be_bytes().to_vec()),
        ] {
            let plane = self
                .planes
                .get_mut(&id)
                .ok_or_else(|| invalid(&name(id), "missing plane"))?;
            let start = index
                .checked_mul(bytes.len())
                .ok_or_else(|| invalid("tile", "offset overflow"))?;
            let end = start
                .checked_add(bytes.len())
                .ok_or_else(|| invalid("tile", "offset overflow"))?;
            plane
                .get_mut(start..end)
                .ok_or_else(|| invalid("tile", "plane too short"))?
                .copy_from_slice(&bytes);
        }
        Ok(())
    }
}

fn select_mut<'a>(
    schema: &TableSchema,
    row: &'a mut TableRecord,
    path: &[PathElement],
) -> Result<&'a mut WireValue, WorldError> {
    let Some((PathElement::Field(field), rest)) = path.split_first() else {
        return Err(invalid("edit", "expected field name"));
    };
    let position = schema
        .fields()
        .iter()
        .position(|s| s.name() == field)
        .ok_or_else(|| invalid(field, "unknown field"))?;
    let descriptor = schema
        .fields()
        .get(position)
        .ok_or_else(|| invalid(field, "unknown descriptor"))?;
    let value = row
        .values_mut()
        .get_mut(position)
        .ok_or_else(|| invalid(field, "missing field value"))?;
    if rest.is_empty() {
        return Ok(value);
    }
    let Some((PathElement::Index(index), rest)) = rest.split_first() else {
        return Err(invalid(field, "expected list index"));
    };
    match value {
        WireValue::Array(values) if rest.is_empty() => values
            .get_mut(*index)
            .ok_or_else(|| invalid(field, "list index out of range")),
        WireValue::Structs(rows) => {
            let child = descriptor
                .child()
                .ok_or_else(|| invalid(field, "missing child schema"))?;
            let row = rows
                .get_mut(*index)
                .ok_or_else(|| invalid(field, "list index out of range"))?;
            select_mut(child, row, rest)
        }
        WireValue::Array(_)
        | WireValue::Signed(_)
        | WireValue::Unsigned(_)
        | WireValue::Bytes(_) => Err(invalid(field, "invalid edit traversal")),
    }
}
