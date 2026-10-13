use super::types::ConsistError;
use crate::world_access;
use ottd_save::{FieldSchema, TableRecord, TableSchema, WireValue};

/// Saved fields used by the consist boundary, without stringly typed errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistField {
    /// Vehicle roster.
    Vehicles,
    /// Vehicle kind.
    Type,
    /// Train subrecord.
    Train,
    /// Common vehicle subrecord.
    Common,
    /// Saved next reference.
    Next,
    /// Train subtype bits.
    Subtype,
    /// Engine identity.
    Engine,
    /// Sprite number.
    Sprite,
    /// Owner identity.
    Owner,
    /// Cargo slot.
    Cargo,
    /// Saved capacity.
    Capacity,
    /// Cargo packets.
    Packets,
    /// Cargo action counts.
    Actions,
    /// Orders reference.
    Orders,
    /// Shared next reference.
    Shared,
    /// Current order type.
    OrderType,
    /// Current speed.
    Speed,
    /// Vehicle status bits.
    Status,
    /// Train flags.
    Flags,
    /// Ground vehicle flags.
    GroundFlags,
    /// Current tile.
    Tile,
    /// Train track bits.
    Track,
    /// Direction.
    Direction,
    /// World pixel X.
    X,
    /// World pixel Y.
    Y,
    /// World pixel height.
    Z,
    /// Train slope setting.
    Slope,
    /// Wagon speed limit setting.
    WagonSpeed,
}
impl ConsistField {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Vehicles => "VEHS",
            Self::Type => "type",
            Self::Train => "train",
            Self::Common => "common",
            Self::Next => "next",
            Self::Subtype => "subtype",
            Self::Engine => "engine_type",
            Self::Sprite => "spritenum",
            Self::Owner => "owner",
            Self::Cargo => "cargo_type",
            Self::Capacity => "cargo_cap",
            Self::Packets => "cargo.packets",
            Self::Actions => "cargo.action_counts",
            Self::Orders => "orders",
            Self::Shared => "next_shared",
            Self::OrderType => "current_order.type",
            Self::Speed => "cur_speed",
            Self::Status => "vehstatus",
            Self::Flags => "flags",
            Self::GroundFlags => "gv_flags",
            Self::Tile => "tile",
            Self::Track => "track",
            Self::Direction => "direction",
            Self::X => "x_pos",
            Self::Y => "y_pos",
            Self::Z => "z_pos",
            Self::Slope => "vehicle.train_slope_steepness",
            Self::WagonSpeed => "vehicle.wagon_speed_limits",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Row<'a> {
    pub(super) schema: &'a TableSchema,
    pub(super) record: &'a TableRecord,
}
impl<'a> Row<'a> {
    pub(super) fn field(self, field: ConsistField) -> Result<&'a WireValue, ConsistError> {
        world_access::row_field(self.schema, self.record, field.name())
            .map_err(|_| ConsistError::Field(field))
    }
    pub(super) fn number<T: TryFrom<u64>>(self, field: ConsistField) -> Result<T, ConsistError> {
        let WireValue::Unsigned(value) = self.field(field)? else {
            return Err(ConsistError::Field(field));
        };
        T::try_from(*value).map_err(|_| ConsistError::Field(field))
    }
    pub(super) fn signed(self, field: ConsistField) -> Result<i32, ConsistError> {
        match self.field(field)? {
            WireValue::Signed(value) => {
                i32::try_from(*value).map_err(|_| ConsistError::Field(field))
            }
            WireValue::Unsigned(_)
            | WireValue::Bytes(_)
            | WireValue::Array(_)
            | WireValue::Structs(_) => Err(ConsistError::Field(field)),
        }
    }
    pub(super) fn single(self, field: ConsistField) -> Result<Self, ConsistError> {
        let schema = self
            .schema
            .fields()
            .iter()
            .find(|f| f.name() == field.name())
            .and_then(FieldSchema::child)
            .ok_or(ConsistError::Field(field))?;
        let WireValue::Structs(rows) = self.field(field)? else {
            return Err(ConsistError::Field(field));
        };
        let [record] = rows.as_slice() else {
            return Err(ConsistError::Field(field));
        };
        Ok(Self { schema, record })
    }
}
