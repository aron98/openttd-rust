use super::{RuntimeError, VehicleId};
use ottd_save::{TableRecord, TableSchema, WireValue, world::World};

#[derive(Debug, Clone, Copy)]
struct Row<'a> {
    schema: &'a TableSchema,
    record: &'a TableRecord,
}
impl<'a> Row<'a> {
    fn field(self, name: &'static str) -> Result<&'a WireValue, RuntimeError> {
        self.schema
            .fields()
            .iter()
            .zip(self.record.values())
            .find_map(|(f, v)| (f.name() == name).then_some(v))
            .ok_or(RuntimeError::Invalid(name))
    }
    fn number(self, name: &'static str) -> Result<u64, RuntimeError> {
        match self.field(name)? {
            WireValue::Unsigned(v) => Ok(*v),
            WireValue::Signed(v) => u64::try_from(*v).map_err(|_| RuntimeError::Invalid(name)),
            _ => Err(RuntimeError::Invalid(name)),
        }
    }
    fn single(self, name: &'static str) -> Result<Self, RuntimeError> {
        let schema = self
            .schema
            .fields()
            .iter()
            .find(|f| f.name() == name)
            .and_then(ottd_save::FieldSchema::child)
            .ok_or(RuntimeError::Invalid(name))?;
        let WireValue::Structs(rows) = self.field(name)? else {
            return Err(RuntimeError::Invalid(name));
        };
        let [record] = rows.as_slice() else {
            return Err(RuntimeError::Invalid(name));
        };
        Ok(Self { schema, record })
    }
}

/// Borrowed saved fields of one vanilla road front; no mutable vehicle mirror.
#[derive(Debug, Clone, Copy)]
pub struct SavedVehicleView<'a> {
    world: &'a World,
    id: VehicleId,
    common: Row<'a>,
}
impl<'a> SavedVehicleView<'a> {
    pub(super) fn new(world: &'a World, id: VehicleId) -> Result<Self, RuntimeError> {
        let table = world
            .tables()
            .get(b"VEHS")
            .ok_or(RuntimeError::Invalid("VEHS"))?;
        let row = Row {
            schema: table.schema(),
            record: table
                .records()
                .get(&id.raw())
                .ok_or(RuntimeError::Invalid("vehicle ID"))?,
        };
        if row.number("type")? != 1 {
            return Err(RuntimeError::Unsupported("non-road vehicle"));
        }
        let common = row.single("roadveh")?.single("common")?;
        if common.number("subtype")? != 1 || common.number("next")? != 0 {
            return Err(RuntimeError::Unsupported(
                "non-front or articulated road vehicle",
            ));
        }
        Ok(Self { world, id, common })
    }
    /// Native sparse pool ID.
    pub const fn id(self) -> VehicleId {
        self.id
    }
    pub(super) const fn world(self) -> &'a World {
        self.world
    }
    /// Saved engine ID.
    /// # Errors
    /// Rejects an invalid saved field width.
    pub fn engine_id(self) -> Result<u16, RuntimeError> {
        u16::try_from(self.common.number("engine_type")?)
            .map_err(|_| RuntimeError::Invalid("engine_type"))
    }
    /// Saved tile index.
    /// # Errors
    /// Rejects an invalid saved field width.
    pub fn tile(self) -> Result<u32, RuntimeError> {
        u32::try_from(self.common.number("tile")?).map_err(|_| RuntimeError::Invalid("tile"))
    }
    /// Saved cargo slot.
    /// # Errors
    /// Rejects an invalid saved field width.
    pub fn cargo_type(self) -> Result<u8, RuntimeError> {
        u8::try_from(self.common.number("cargo_type")?)
            .map_err(|_| RuntimeError::Invalid("cargo_type"))
    }
    /// Saved capacity, never regenerated from the engine during load.
    /// # Errors
    /// Rejects an invalid saved field width.
    pub fn capacity(self) -> Result<u16, RuntimeError> {
        u16::try_from(self.common.number("cargo_cap")?)
            .map_err(|_| RuntimeError::Invalid("cargo_cap"))
    }
    /// Cargo physically aboard, excluding reserved-to-load packets.
    /// # Errors
    /// Rejects malformed native action counts.
    pub fn stored_count(self) -> Result<u32, RuntimeError> {
        let WireValue::Array(actions) = self.common.field("cargo.action_counts")? else {
            return Err(RuntimeError::Invalid("cargo.action_counts"));
        };
        if actions.len() != 4 {
            return Err(RuntimeError::Invalid("cargo.action_counts"));
        }
        actions.iter().take(3).try_fold(0_u32, |count, action| {
            let WireValue::Unsigned(value) = action else {
                return Err(RuntimeError::Invalid("cargo.action_counts"));
            };
            Ok(count.wrapping_add(
                u32::try_from(*value).map_err(|_| RuntimeError::Invalid("cargo.action_counts"))?,
            ))
        })
    }
    /// Saved internal speed.
    /// # Errors
    /// Rejects an invalid saved field width.
    pub fn current_speed(self) -> Result<u16, RuntimeError> {
        u16::try_from(self.common.number("cur_speed")?)
            .map_err(|_| RuntimeError::Invalid("cur_speed"))
    }
    /// Saved reliability; restoration does not reseed it.
    /// # Errors
    /// Rejects an invalid saved field width.
    pub fn reliability(self) -> Result<u16, RuntimeError> {
        u16::try_from(self.common.number("reliability")?)
            .map_err(|_| RuntimeError::Invalid("reliability"))
    }
    /// Saved lifetime assigned when built, in days.
    /// # Errors
    /// Rejects malformed saved age.
    pub fn max_age(self) -> Result<i32, RuntimeError> {
        match self.common.field("max_age")? {
            WireValue::Signed(v) => i32::try_from(*v).map_err(|_| RuntimeError::Invalid("max_age")),
            _ => Err(RuntimeError::Invalid("max_age")),
        }
    }
}
