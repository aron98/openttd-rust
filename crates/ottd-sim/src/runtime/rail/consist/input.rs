use super::{
    fields::{ConsistField as F, Row},
    geometry::TrackContext,
    types::{ConsistError, ConsistRefusal},
};
use crate::{
    content::{ContentCatalog, EngineInfo, RailSpec, VehicleSpec},
    runtime::VehicleId,
    world_access,
};
use ottd_save::{WireValue, world::World};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) struct Part {
    pub id: VehicleId,
    pub next: Option<VehicleId>,
    pub subtype: u8,
    pub engine: u16,
    pub sprite: u8,
    pub owner: u8,
    pub capacity: u16,
    pub speed: u16,
    pub status: u8,
    pub flags: u16,
    pub ground_flags: u16,
    pub direction: u8,
    pub track: u8,
    pub position: [i32; 3],
    pub context: TrackContext,
    pub spec: RailSpec,
    pub info: EngineInfo,
}
#[derive(Clone, Copy)]
pub(super) struct Settings {
    pub slope: u8,
    pub wagon_speed: bool,
}

pub(super) fn read(
    world: &World,
    content: &ContentCatalog,
) -> Result<(BTreeMap<VehicleId, Part>, Settings), ConsistError> {
    let table = world
        .tables()
        .get(b"VEHS")
        .ok_or(ConsistError::Field(F::Vehicles))?;
    if table.records().is_empty() {
        return Err(ConsistRefusal::Roster.into());
    }
    for chunk in [b"ORDL", b"BKOR"] {
        if world
            .tables()
            .get(chunk)
            .is_none_or(|table| !table.records().is_empty())
        {
            return Err(ConsistRefusal::Orders.into());
        }
    }
    let mut parts = BTreeMap::new();
    for (id, record) in table.records() {
        let row = Row {
            schema: table.schema(),
            record,
        };
        if row.number::<u8>(F::Type)? != 0 {
            return Err(ConsistRefusal::Roster.into());
        }
        let train = row.single(F::Train)?;
        let common = train.single(F::Common)?;
        if common.number::<u32>(F::Orders)? != 0
            || common.number::<u32>(F::Shared)? != 0
            || common.number::<u8>(F::OrderType)? != 0
        {
            return Err(ConsistRefusal::Orders.into());
        }
        let WireValue::Array(packets) = common.field(F::Packets)? else {
            return Err(ConsistError::Field(F::Packets));
        };
        let WireValue::Array(actions) = common.field(F::Actions)? else {
            return Err(ConsistError::Field(F::Actions));
        };
        let cargo = common.number::<u8>(F::Cargo)?;
        if !packets.is_empty()
            || actions.len() != 4
            || actions.iter().any(|v| *v != WireValue::Unsigned(0))
            || content
                .cargo()
                .get(usize::from(cargo))
                .is_none_or(|spec| spec.bitnum == 255)
        {
            return Err(ConsistRefusal::Cargo.into());
        }
        let engine = common.number::<u16>(F::Engine)?;
        let (spec, info) = specification(content, engine)?;
        let position = [
            i32::try_from(common.number::<u32>(F::X)?).map_err(|_| ConsistError::Field(F::X))?,
            i32::try_from(common.number::<u32>(F::Y)?).map_err(|_| ConsistError::Field(F::Y))?,
            common.signed(F::Z)?,
        ];
        let id = VehicleId::new(*id);
        parts.insert(
            id,
            Part {
                id,
                next: common
                    .number::<u32>(F::Next)?
                    .checked_sub(1)
                    .map(VehicleId::new),
                subtype: common.number(F::Subtype)?,
                engine,
                sprite: common.number(F::Sprite)?,
                owner: common.number(F::Owner)?,
                capacity: common.number(F::Capacity)?,
                speed: common.number(F::Speed)?,
                status: common.number(F::Status)?,
                flags: train.number(F::Flags)?,
                ground_flags: train.number(F::GroundFlags)?,
                direction: common.number(F::Direction)?,
                track: train.number(F::Track)?,
                position,
                context: TrackContext::read(world, common.number(F::Tile)?, position)?,
                spec,
                info,
            },
        );
    }
    Ok((parts, Settings::read(world)?))
}

impl Settings {
    fn read(world: &World) -> Result<Self, ConsistError> {
        let slope = world_access::unsigned(world, b"PATS", 0, F::Slope.name())
            .map_err(|_| ConsistError::Field(F::Slope))?;
        let wagon = world_access::unsigned(world, b"PATS", 0, F::WagonSpeed.name())
            .map_err(|_| ConsistError::Field(F::WagonSpeed))?;
        if wagon > 1 {
            return Err(ConsistError::Field(F::WagonSpeed));
        }
        Ok(Self {
            slope: u8::try_from(slope).map_err(|_| ConsistError::Field(F::Slope))?,
            wagon_speed: wagon != 0,
        })
    }
}

fn specification(
    content: &ContentCatalog,
    engine: u16,
) -> Result<(RailSpec, EngineInfo), ConsistError> {
    let catalog = content
        .engines()
        .get(usize::from(engine))
        .ok_or(ConsistRefusal::Engine)?;
    let spec = match catalog.vehicle {
        VehicleSpec::Rail(spec) => spec,
        VehicleSpec::Road(_) | VehicleSpec::Ship(_) | VehicleSpec::Aircraft(_) => {
            return Err(ConsistRefusal::Engine.into());
        }
    };
    if spec.railtypes != 1
        || spec.engclass > 1
        || spec.visual_effect != 255
        || catalog.info.callback_mask != 0
        || spec.pow_wag_power != 0
        || spec.pow_wag_weight != 0
    {
        return Err(ConsistRefusal::Engine.into());
    }
    Ok((spec, catalog.info))
}
