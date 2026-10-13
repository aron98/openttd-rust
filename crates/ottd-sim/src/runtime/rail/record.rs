use super::{RailRefusal, RailRestoreError};
use crate::{
    content::{ContentCatalog, EngineInfo, RailSpec, VehicleSpec},
    runtime::VehicleId,
    world_access,
};
use ottd_save::{FieldSchema, TableRecord, TableSchema, WireValue, world::World};

pub(super) struct RailInput {
    pub id: VehicleId,
    pub spec: RailSpec,
    pub info: EngineInfo,
    pub flags: u16,
    pub speed: u16,
    pub capacity: u16,
    pub slope: u8,
}

#[derive(Clone, Copy)]
struct Row<'a> {
    schema: &'a TableSchema,
    record: &'a TableRecord,
}
impl<'a> Row<'a> {
    fn field(self, name: &'static str) -> Result<&'a WireValue, RailRestoreError> {
        world_access::row_field(self.schema, self.record, name)
            .map_err(|_| RailRestoreError::Invalid(name))
    }
    fn number<T: TryFrom<u64>>(self, name: &'static str) -> Result<T, RailRestoreError> {
        let WireValue::Unsigned(value) = self.field(name)? else {
            return Err(RailRestoreError::Invalid(name));
        };
        T::try_from(*value).map_err(|_| RailRestoreError::Invalid(name))
    }
    fn single(self, name: &'static str) -> Result<Self, RailRestoreError> {
        let schema = self
            .schema
            .fields()
            .iter()
            .find(|field| field.name() == name)
            .and_then(FieldSchema::child)
            .ok_or(RailRestoreError::Invalid(name))?;
        let WireValue::Structs(rows) = self.field(name)? else {
            return Err(RailRestoreError::Invalid(name));
        };
        let [record] = rows.as_slice() else {
            return Err(RailRestoreError::Invalid(name));
        };
        Ok(Self { schema, record })
    }
}

pub(super) fn read(world: &World, content: &ContentCatalog) -> Result<RailInput, RailRestoreError> {
    let table = world
        .tables()
        .get(b"VEHS")
        .ok_or(RailRestoreError::Invalid("VEHS"))?;
    if table.records().len() != 1 {
        return Err(RailRefusal::VehicleRoster.into());
    }
    let (id, record) = table
        .records()
        .first_key_value()
        .ok_or(RailRefusal::VehicleRoster)?;
    let row = Row {
        schema: table.schema(),
        record,
    };
    if row.number::<u8>("type")? != 0 {
        return Err(RailRefusal::VehicleRoster.into());
    }
    let train = row.single("train")?;
    let common = train.single("common")?;
    if common.number::<u8>("subtype")? != 9 || common.number::<u32>("next")? != 0 {
        return Err(RailRefusal::Consist.into());
    }
    for chunk in [b"ORDL", b"BKOR"] {
        if world
            .tables()
            .get(chunk)
            .is_none_or(|table| !table.records().is_empty())
        {
            return Err(RailRefusal::Orders.into());
        }
    }
    if common.number::<u32>("orders")? != 0
        || common.number::<u32>("next_shared")? != 0
        || common.number::<u8>("current_order.type")? != 0
    {
        return Err(RailRefusal::Orders.into());
    }
    let WireValue::Array(packets) = common.field("cargo.packets")? else {
        return Err(RailRestoreError::Invalid("cargo.packets"));
    };
    let WireValue::Array(actions) = common.field("cargo.action_counts")? else {
        return Err(RailRestoreError::Invalid("cargo.action_counts"));
    };
    if !packets.is_empty()
        || actions.len() != 4
        || actions.iter().any(|v| *v != WireValue::Unsigned(0))
    {
        return Err(RailRefusal::Cargo.into());
    }
    let cargo = common.number::<u8>("cargo_type")?;
    if content
        .cargo()
        .get(usize::from(cargo))
        .is_none_or(|spec| spec.bitnum == 255)
    {
        return Err(RailRestoreError::Invalid("cargo_type"));
    }
    let speed = common.number::<u16>("cur_speed")?;
    let status = common.number::<u8>("vehstatus")?;
    if speed != 0 || status & 2 == 0 || status & 0x81 != 0 {
        return Err(RailRefusal::Motion.into());
    }
    let engine = content
        .engines()
        .get(usize::from(common.number::<u16>("engine_type")?))
        .ok_or(RailRestoreError::Invalid("engine_type"))?;
    let spec = match engine.vehicle {
        VehicleSpec::Rail(spec) => spec,
        VehicleSpec::Road(_) | VehicleSpec::Ship(_) | VehicleSpec::Aircraft(_) => {
            return Err(RailRefusal::Engine.into());
        }
    };
    if spec.railveh_type != 0
        || spec.railtypes != 1
        || spec.power == 0
        || spec.capacity != 0
        || spec.engclass > 1
        || spec.visual_effect != 255
        || engine.info.callback_mask != 0
    {
        return Err(RailRefusal::Engine.into());
    }
    validate_track(world, train, common)?;
    let flags = train.number::<u16>("flags")?;
    if flags & !8 != 0 {
        return Err(RailRefusal::Consist.into());
    }
    let slope = world_access::unsigned(world, b"PATS", 0, "vehicle.train_slope_steepness")
        .map_err(|_| RailRestoreError::Invalid("vehicle.train_slope_steepness"))?;
    Ok(RailInput {
        id: VehicleId::new(*id),
        spec,
        info: engine.info,
        flags,
        speed,
        capacity: common.number("cargo_cap")?,
        slope: u8::try_from(slope)
            .map_err(|_| RailRestoreError::Invalid("vehicle.train_slope_steepness"))?,
    })
}

fn validate_track(world: &World, train: Row<'_>, common: Row<'_>) -> Result<(), RailRestoreError> {
    let tile_id = common.number::<u32>("tile")?;
    let map = world.map();
    let index = usize::try_from(tile_id).map_err(|_| RailRestoreError::Invalid("tile"))?;
    let tile = map
        .tiles()
        .get(index)
        .ok_or(RailRestoreError::Invalid("tile"))?;
    let track = train.number::<u8>("track")?;
    let direction = common.number::<u8>("direction")?;
    let matching_direction = match track {
        1 => matches!(direction, 1 | 5),
        2 => matches!(direction, 3 | 7),
        _ => false,
    };
    if tile.tile_type() >> 4 != 1
        || tile.m5() != track
        || tile.m8() & 63 != 0
        || !matching_direction
        || tile.m1() & 31 != common.number::<u8>("owner")?
        || train.number::<u16>("gv_flags")? & 3 != 0
    {
        return Err(RailRefusal::Track.into());
    }
    for offset in [1, map.width(), map.width().saturating_add(1)] {
        let neighbour = tile_id
            .checked_add(offset)
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|id| map.tiles().get(id));
        if neighbour.is_none_or(|other| other.height() != tile.height()) {
            return Err(RailRefusal::Track.into());
        }
    }
    Ok(())
}
