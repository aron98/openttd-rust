use super::{RuntimeError, VehicleId, properties, view};
use crate::content::ContentCatalog;
use ottd_save::{
    WireValue,
    world::{CandidateView, World, WorldEdit},
};

pub(super) struct Shared {
    pub(super) list: u32,
    pub(super) clone: u32,
    next: u64,
}
impl Shared {
    pub(super) fn update_chain(
        &self,
        lists: &mut [ottd_save::world::OrderListState],
        vehicle: VehicleId,
    ) -> Result<(), RuntimeError> {
        let list = lists
            .iter_mut()
            .find(|list| list.id == self.list)
            .ok_or(RuntimeError::Invalid("shared restore derived list"))?;
        let position = list
            .vehicles
            .iter()
            .position(|id| *id == self.clone)
            .ok_or(RuntimeError::Invalid("shared restore chain member"))?;
        let next = position
            .checked_add(1)
            .ok_or(RuntimeError::Invalid("shared restore chain position"))?;
        list.vehicles.insert(next, vehicle.raw());
        Ok(())
    }
}

fn row(
    reader: view::OrderReader<'_>,
    chunk: [u8; 4],
    id: u32,
) -> Result<view::Row<'_>, RuntimeError> {
    reader
        .rows(chunk)?
        .into_iter()
        .find_map(|(key, row)| (key == id).then_some(row))
        .ok_or(RuntimeError::Unsupported("shared restore source record"))
}
fn bus(cargo: u64, content: &ContentCatalog) -> Result<bool, RuntimeError> {
    let id = usize::try_from(cargo).map_err(|_| RuntimeError::Invalid("shared restore cargo"))?;
    Ok(content
        .cargo()
        .get(id)
        .ok_or(RuntimeError::Invalid("shared restore cargo"))?
        .classes
        & 1
        != 0)
}
fn validate(
    reader: view::OrderReader<'_>,
    backup: view::Row<'_>,
    owner: u64,
    cargo: u64,
    content: &ContentCatalog,
) -> Result<Shared, RuntimeError> {
    let clone = u32::try_from(
        backup
            .number("clone")?
            .checked_sub(1)
            .ok_or(RuntimeError::Invalid("shared restore clone"))?,
    )
    .map_err(|_| RuntimeError::Invalid("shared restore clone"))?;
    let Some(("roadveh", source)) = view::vehicle(row(reader, *b"VEHS", clone)?)? else {
        return Err(RuntimeError::Unsupported("shared restore road primary"));
    };
    if source.number("subtype")? != 1
        || source.number("next")? != 0
        || source.number("owner")? != owner
        || bus(source.number("cargo_type")?, content)? != bus(cargo, content)?
    {
        return Err(RuntimeError::Unsupported(
            "shared restore owner or road class",
        ));
    }
    let list = u32::try_from(source.number("orders")?.checked_sub(1).ok_or(
        RuntimeError::Unsupported("shared restore missing order list"),
    )?)
    .map_err(|_| RuntimeError::Invalid("shared restore list"))?;
    let (schema, orders) = row(reader, *b"ORDL", list)?.children("orders")?;
    for record in orders {
        if (view::Row { schema, record }).number("type")? & 15 == 1 {
            return Err(RuntimeError::Unsupported("shared restore station orders"));
        }
    }
    Ok(Shared {
        list,
        clone,
        next: source.number("next_shared")?,
    })
}
pub(in crate::runtime) fn admit(
    world: &World,
    slot: u32,
    owner: u8,
    cargo: u8,
    content: &ContentCatalog,
) -> Result<(), RuntimeError> {
    let backup = view::row(world, *b"BKOR", slot)?;
    if backup.number("clone")? != 0 {
        let _ = validate(
            view::OrderReader::Committed(world),
            backup,
            u64::from(owner),
            u64::from(cargo),
            content,
        )?;
    }
    Ok(())
}
pub(super) fn stage(
    candidate: CandidateView<'_>,
    backup: view::Row<'_>,
    current: view::Row<'_>,
    vehicle: VehicleId,
    content: &ContentCatalog,
) -> Result<(Shared, Vec<WorldEdit>), RuntimeError> {
    if current.number("subtype")? != 1 || current.number("next")? != 0 {
        return Err(RuntimeError::Invalid("shared restore new primary"));
    }
    let shared = validate(
        view::OrderReader::Candidate(candidate),
        backup,
        current.number("owner")?,
        current.number("cargo_type")?,
        content,
    )?;
    if shared.clone == vehicle.raw() {
        return Err(RuntimeError::Invalid("shared restore self clone"));
    }
    let edits = vec![
        properties::field(
            vehicle,
            "orders",
            WireValue::Unsigned(u64::from(shared.list) + 1),
        ),
        properties::field(vehicle, "next_shared", WireValue::Unsigned(shared.next)),
        properties::field(
            VehicleId::new(shared.clone),
            "next_shared",
            WireValue::Unsigned(u64::from(vehicle.raw()) + 1),
        ),
        properties::field(
            vehicle,
            "depot_unbunching_last_departure",
            WireValue::Unsigned(0),
        ),
        properties::field(
            vehicle,
            "depot_unbunching_next_departure",
            WireValue::Unsigned(0),
        ),
        properties::field(vehicle, "round_trip_time", WireValue::Signed(0)),
    ];
    Ok((shared, edits))
}
