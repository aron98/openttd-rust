use super::{RuntimeError, VehicleId};
use ottd_save::{
    TableRecord, TableSchema, TileState, WireValue,
    world::{CandidateView, World},
};

#[derive(Debug, Clone, Copy)]
enum Source<'a> {
    Committed(&'a World),
    Candidate(CandidateView<'a>),
}
impl<'a> Source<'a> {
    fn row(self, chunk: [u8; 4], id: u32) -> Result<Row<'a>, RuntimeError> {
        let (schema, record) = match self {
            Self::Committed(world) => {
                let table = world
                    .tables()
                    .get(&chunk)
                    .ok_or(RuntimeError::Invalid("runtime table"))?;
                (table.schema(), table.records().get(&id))
            }
            Self::Candidate(view) => {
                let table = view
                    .table(chunk)
                    .ok_or(RuntimeError::Invalid("runtime table"))?;
                (table.schema(), table.record(id))
            }
        };
        Ok(Row {
            schema,
            record: record.ok_or(RuntimeError::Invalid("runtime record"))?,
        })
    }
    fn tile(self, id: u32) -> Result<TileState, RuntimeError> {
        match self {
            Self::Committed(world) => usize::try_from(id)
                .ok()
                .and_then(|id| world.map().tiles().get(id))
                .cloned()
                .ok_or(RuntimeError::Invalid("vehicle tile")),
            Self::Candidate(view) => view
                .tile(id)
                .map_err(|_| RuntimeError::Invalid("vehicle tile")),
        }
    }
}

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
    source: Source<'a>,
    id: VehicleId,
    common: Row<'a>,
}
impl<'a> SavedVehicleView<'a> {
    pub(super) fn common_field(self, name: &'static str) -> Result<&'a WireValue, RuntimeError> {
        self.common.field(name)
    }
    pub(super) fn common_number(self, name: &'static str) -> Result<u64, RuntimeError> {
        self.common.number(name)
    }
    pub(super) fn common_signed(self, name: &'static str) -> Result<i64, RuntimeError> {
        match self.common.field(name)? {
            WireValue::Signed(value) => Ok(*value),
            _ => Err(RuntimeError::Invalid(name)),
        }
    }
    pub(super) fn road_field(self, name: &'static str) -> Result<&'a WireValue, RuntimeError> {
        self.source
            .row(*b"VEHS", self.id.raw())?
            .single("roadveh")?
            .field(name)
    }
    pub(super) fn road_state(self) -> Result<u64, RuntimeError> {
        self.source
            .row(*b"VEHS", self.id.raw())?
            .single("roadveh")?
            .number("state")
    }
    pub(super) fn new(world: &'a World, id: VehicleId) -> Result<Self, RuntimeError> {
        Self::read(Source::Committed(world), id)
    }
    pub(super) fn candidate(view: CandidateView<'a>, id: VehicleId) -> Result<Self, RuntimeError> {
        Self::read(Source::Candidate(view), id)
    }
    fn read(source: Source<'a>, id: VehicleId) -> Result<Self, RuntimeError> {
        let row = source.row(*b"VEHS", id.raw())?;
        if row.number("type")? != 1 {
            return Err(RuntimeError::Unsupported("non-road vehicle"));
        }
        let common = row.single("roadveh")?.single("common")?;
        if common.number("subtype")? != 1 || common.number("next")? != 0 {
            return Err(RuntimeError::Unsupported(
                "non-front or articulated road vehicle",
            ));
        }
        Ok(Self { source, id, common })
    }
    /// Native sparse pool ID.
    pub const fn id(self) -> VehicleId {
        self.id
    }
    pub(super) fn owner(self) -> Result<u8, RuntimeError> {
        u8::try_from(self.common.number("owner")?).map_err(|_| RuntimeError::Invalid("owner"))
    }
    pub(super) fn unit_number(self) -> Result<u16, RuntimeError> {
        u16::try_from(self.common.number("unitnumber")?)
            .map_err(|_| RuntimeError::Invalid("unitnumber"))
    }
    pub(super) fn cargo_paid_for(self) -> Result<u16, RuntimeError> {
        u16::try_from(self.common.number("cargo_paid_for")?)
            .map_err(|_| RuntimeError::Invalid("cargo_paid_for"))
    }
    pub(super) fn tile_state(self) -> Result<TileState, RuntimeError> {
        self.source.tile(self.tile()?)
    }
    pub(super) fn setting(self, name: &'static str) -> Result<u32, RuntimeError> {
        u32::try_from(self.source.row(*b"PATS", 0)?.number(name)?)
            .map_err(|_| RuntimeError::Invalid(name))
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
