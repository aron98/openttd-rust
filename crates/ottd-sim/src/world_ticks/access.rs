#![expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "chunk keys are borrowed directly for table lookups"
)]
use super::{WorldTickError, unsupported};
use crate::world_access::{field_edit, row_field};
use ottd_save::{
    TableChunk, TableRecord, TableSchema, WireValue,
    world::{PathElement, World, WorldEdit},
};
use std::collections::BTreeMap;

pub(super) struct State {
    pub tables: BTreeMap<[u8; 4], TableChunk>,
}
impl State {
    pub(super) fn random_state(&self) -> Result<[u32; 2], WorldTickError> {
        Ok([
            u32::try_from(self.number(b"DATE", 0, "random_state[0]")?)
                .map_err(|_| unsupported("random", "state word"))?,
            u32::try_from(self.number(b"DATE", 0, "random_state[1]")?)
                .map_err(|_| unsupported("random", "state word"))?,
        ])
    }
    pub(super) fn load(world: &World) -> Self {
        Self {
            tables: world
                .tables()
                .iter()
                .filter(|(id, _)| matches!(*id, b"DATE" | b"PLYR" | b"ECMY" | b"IBLD" | b"CITY"))
                .map(|(id, t)| (*id, t.clone()))
                .collect(),
        }
    }
    pub(super) fn value(
        &self,
        id: &[u8; 4],
        record: u32,
        name: &str,
    ) -> Result<&WireValue, WorldTickError> {
        let table = self
            .tables
            .get(id)
            .ok_or_else(|| unsupported("state", "missing table"))?;
        let row = table
            .records()
            .get(&record)
            .ok_or_else(|| unsupported("state", "missing row"))?;
        Ok(row_field(table.schema(), row, name)?)
    }
    pub(super) fn number(
        &self,
        id: &[u8; 4],
        record: u32,
        name: &str,
    ) -> Result<i64, WorldTickError> {
        number(self.value(id, record, name)?)
    }
    pub(super) fn set(
        &mut self,
        id: &[u8; 4],
        record: u32,
        name: &str,
        value: WireValue,
    ) -> Result<(), WorldTickError> {
        let table = self
            .tables
            .get_mut(id)
            .ok_or_else(|| unsupported("state", "missing table"))?;
        let schema = table.schema().clone();
        let row = table
            .records_mut()
            .get_mut(&record)
            .ok_or_else(|| unsupported("state", "missing row"))?;
        *value_mut(&schema, row, name)? = value;
        Ok(())
    }
    pub(super) fn set_number(
        &mut self,
        id: &[u8; 4],
        record: u32,
        name: &str,
        value: i64,
    ) -> Result<(), WorldTickError> {
        let next = match self.value(id, record, name)? {
            WireValue::Signed(_) => WireValue::Signed(value),
            WireValue::Unsigned(_) => {
                WireValue::Unsigned(u64::try_from(value).map_err(|_| unsupported("state", name))?)
            }
            _ => return Err(unsupported("state", name)),
        };
        self.set(id, record, name, next)
    }
    pub(super) fn company_ids(&self) -> Vec<u32> {
        self.tables
            .get(b"PLYR")
            .map(|t| t.records().keys().copied().collect())
            .unwrap_or_default()
    }
    pub(super) fn edits(self, world: &World) -> Result<Vec<WorldEdit>, WorldTickError> {
        let mut edits = Vec::new();
        for (id, table) in self.tables {
            for (index, row) in table.records() {
                for (descriptor, value) in table.schema().fields().iter().zip(row.values()) {
                    if crate::world_access::field(world, &id, *index, descriptor.name())? == value {
                        continue;
                    }
                    edits.push(match value {
                        WireValue::Structs(rows) => WorldEdit::StructList {
                            chunk: id,
                            record: *index,
                            path: vec![PathElement::Field(descriptor.name().into())],
                            rows: rows.clone(),
                        },
                        _ => field_edit(id, *index, descriptor.name(), value.clone()),
                    });
                }
            }
        }
        Ok(edits)
    }
}
pub(super) fn number(value: &WireValue) -> Result<i64, WorldTickError> {
    match value {
        WireValue::Signed(v) => Ok(*v),
        WireValue::Unsigned(v) => {
            i64::try_from(*v).map_err(|_| unsupported("state", "integer range"))
        }
        _ => Err(unsupported("state", "integer shape")),
    }
}
pub(super) fn value_mut<'a>(
    schema: &TableSchema,
    row: &'a mut TableRecord,
    name: &str,
) -> Result<&'a mut WireValue, WorldTickError> {
    let index = schema
        .fields()
        .iter()
        .position(|f| f.name() == name)
        .ok_or_else(|| unsupported("state", name))?;
    row.values_mut()
        .get_mut(index)
        .ok_or_else(|| unsupported("state", name))
}
