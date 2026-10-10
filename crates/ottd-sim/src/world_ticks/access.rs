#![expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "chunk keys are borrowed directly for table lookups"
)]
use super::{WorldTickError, unsupported};
use crate::world_access::{field_edit, row_field};
use ottd_save::{
    TableRecord, TableSchema, WireValue,
    world::{
        CandidateTable, CandidateView, PathElement, PreparedWorldTransaction, World, WorldEdit,
    },
};

use crate::WorldTickState as State;
impl<'w> State<'w> {
    pub(crate) fn number_value(value: &WireValue) -> Result<i64, WorldTickError> {
        number(value)
    }

    pub(crate) const fn view(&self) -> CandidateView<'_> {
        self.transaction.view()
    }
    pub(crate) fn apply(&mut self, edit: WorldEdit) -> Result<(), WorldTickError> {
        Ok(self.transaction.apply(edit)?)
    }
    pub(crate) fn finish(self) -> Result<PreparedWorldTransaction<'w>, WorldTickError> {
        Ok(self.transaction.prepare()?)
    }
    pub(crate) fn table(&self, id: &[u8; 4]) -> Result<CandidateTable<'_>, WorldTickError> {
        self.view()
            .table(*id)
            .ok_or_else(|| unsupported("state", "missing table"))
    }
    pub(crate) fn random_state(&self) -> Result<[u32; 2], WorldTickError> {
        Ok([
            u32::try_from(self.number(b"DATE", 0, "random_state[0]")?)
                .map_err(|_| unsupported("random", "state word"))?,
            u32::try_from(self.number(b"DATE", 0, "random_state[1]")?)
                .map_err(|_| unsupported("random", "state word"))?,
        ])
    }
    pub(crate) const fn load(world: &'w mut World) -> Self {
        let width = world.map().width();
        let height = world.map().height();
        Self {
            transaction: world.transaction(),
            width,
            height,
        }
    }
    pub(crate) fn value(
        &self,
        id: &[u8; 4],
        record: u32,
        name: &str,
    ) -> Result<&WireValue, WorldTickError> {
        let table = self.table(id)?;
        let row = table
            .record(record)
            .ok_or_else(|| unsupported("state", "missing row"))?;
        Ok(row_field(table.schema(), row, name)?)
    }
    pub(crate) fn number(
        &self,
        id: &[u8; 4],
        record: u32,
        name: &str,
    ) -> Result<i64, WorldTickError> {
        number(self.value(id, record, name)?)
    }
    pub(crate) fn set(
        &mut self,
        id: &[u8; 4],
        record: u32,
        name: &str,
        value: WireValue,
    ) -> Result<(), WorldTickError> {
        let path = vec![PathElement::Field(name.into())];
        let edit = match value {
            WireValue::Structs(rows) => WorldEdit::StructList {
                chunk: *id,
                record,
                path,
                rows,
            },
            value => field_edit(*id, record, name, value),
        };
        self.apply(edit)
    }
    pub(crate) fn set_number(
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
    pub(crate) fn company_ids(&self) -> Vec<u32> {
        self.view()
            .table(*b"PLYR")
            .map(|t| t.records().map(|(id, _)| id).collect())
            .unwrap_or_default()
    }
    pub(crate) fn unsigned(
        &self,
        id: &[u8; 4],
        record: u32,
        name: &str,
    ) -> Result<u64, WorldTickError> {
        u64::try_from(self.number(id, record, name)?).map_err(|_| unsupported("state", name))
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
