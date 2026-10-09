use super::{TableRecord, WireValue, WorldEdit, WorldError, edit, invalid};

impl super::WorldTransaction<'_> {
    pub(super) fn stage(&mut self, edit: WorldEdit) -> Result<(), WorldError> {
        match edit {
            WorldEdit::Tile { index, value } => {
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= self.world.map().tiles().len())
                {
                    return Err(invalid("tile", "index out of range"));
                }
                self.tiles.insert(index, value);
            }
            WorldEdit::InsertRecord {
                chunk,
                record,
                value,
            } => {
                self.require_pool(chunk)?;
                if self
                    .view()
                    .table(chunk)
                    .and_then(|table| table.record(record))
                    .is_some()
                {
                    return Err(invalid(&super::super::name(chunk), "record already exists"));
                }
                self.records
                    .entry(chunk)
                    .or_default()
                    .insert(record, Some(value));
            }
            WorldEdit::RemoveRecord { chunk, record } => {
                self.require_pool(chunk)?;
                self.require_record(chunk, record)?;
                self.records.entry(chunk).or_default().insert(record, None);
            }
            WorldEdit::ReplaceRecord {
                chunk,
                record,
                value,
            } => {
                self.require_pool(chunk)?;
                self.require_record(chunk, record)?;
                self.records
                    .entry(chunk)
                    .or_default()
                    .insert(record, Some(value));
            }
            WorldEdit::Field {
                chunk,
                record,
                path,
                value,
            } => {
                let (schema, row) = self.row_mut(chunk, record)?;
                let target = edit::select_mut(schema, row, &path)?;
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
            }
            WorldEdit::StructList {
                chunk,
                record,
                path,
                rows,
            } => {
                let (schema, row) = self.row_mut(chunk, record)?;
                let target = edit::select_mut(schema, row, &path)?;
                if !matches!(target, WireValue::Structs(_)) {
                    return Err(invalid("edit", "expected structure list"));
                }
                *target = WireValue::Structs(rows);
            }
        }
        Ok(())
    }
    fn require_pool(&self, chunk: [u8; 4]) -> Result<(), WorldError> {
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
                &super::super::name(chunk),
                "record operation requires a native pool",
            ));
        }
        if !self.world.tables.contains_key(&chunk) {
            return Err(invalid(&super::super::name(chunk), "missing table"));
        }
        Ok(())
    }
    fn require_record(&self, chunk: [u8; 4], record: u32) -> Result<(), WorldError> {
        let table = self
            .view()
            .table(chunk)
            .ok_or_else(|| invalid(&super::super::name(chunk), "missing table"))?;
        table
            .record(record)
            .ok_or_else(|| invalid(&super::super::name(chunk), "missing record"))?;
        Ok(())
    }
    fn row_mut(
        &mut self,
        chunk: [u8; 4],
        record: u32,
    ) -> Result<(&crate::TableSchema, &mut TableRecord), WorldError> {
        let table = self
            .world
            .tables
            .get(&chunk)
            .ok_or_else(|| invalid(&super::super::name(chunk), "missing table"))?;
        let rows = self.records.entry(chunk).or_default();
        if let std::collections::btree_map::Entry::Vacant(entry) = rows.entry(record) {
            let original = table
                .records()
                .get(&record)
                .ok_or_else(|| invalid(&super::super::name(chunk), "missing record"))?;
            entry.insert(Some(original.clone()));
        }
        let row = rows
            .get_mut(&record)
            .and_then(Option::as_mut)
            .ok_or_else(|| invalid(&super::super::name(chunk), "missing record"))?;
        Ok((table.schema(), row))
    }
}
