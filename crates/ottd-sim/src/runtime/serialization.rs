use super::{RuntimeError, SavedVehicleView, VehicleId};
use ottd_save::world::World;

pub(super) fn restore(world: &World) -> Result<u16, RuntimeError> {
    let table = world
        .tables()
        .get(b"VEHS")
        .ok_or(RuntimeError::Invalid("VEHS"))?;
    let mut value = None;
    for id in table.records().keys() {
        let saved = SavedVehicleView::new(world, VehicleId::new(*id))?.cargo_paid_for()?;
        if value.is_some_and(|previous| previous != saved) {
            return Err(RuntimeError::Unsupported(
                "nonuniform native cargo_paid_for serialization global",
            ));
        }
        value = Some(saved);
    }
    Ok(value.unwrap_or(0))
}
