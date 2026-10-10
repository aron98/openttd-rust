use super::{SavedVehicleView, State, VehicleId, WireValue, WorldTickError, unsupported};
use ottd_save::world::{PathElement, WorldEdit};

pub(super) fn value<'a>(
    state: &'a State<'_>,
    id: VehicleId,
    road: bool,
    name: &'static str,
) -> Result<&'a WireValue, WorldTickError> {
    let vehicle = SavedVehicleView::candidate(state.view(), id)?;
    Ok(if road {
        vehicle.road_field(name)?
    } else {
        vehicle.common_field(name)?
    })
}
pub(super) fn number(
    state: &State<'_>,
    id: VehicleId,
    road: bool,
    name: &'static str,
) -> Result<i64, WorldTickError> {
    State::number_value(value(state, id, road, name)?)
}
pub(super) fn set(
    state: &mut State<'_>,
    id: VehicleId,
    road: bool,
    name: &'static str,
    next: i64,
) -> Result<(), WorldTickError> {
    let value = match value(state, id, road, name)? {
        WireValue::Signed(_) => WireValue::Signed(next),
        WireValue::Unsigned(_) => {
            WireValue::Unsigned(u64::try_from(next).map_err(|_| unsupported("road", name))?)
        }
        _ => return Err(unsupported("road", name)),
    };
    let mut path = vec![PathElement::Field("roadveh".into()), PathElement::Index(0)];
    if !road {
        path.extend([PathElement::Field("common".into()), PathElement::Index(0)]);
    }
    path.push(PathElement::Field(name.into()));
    state.apply(WorldEdit::Field {
        chunk: *b"VEHS",
        record: id.raw(),
        path,
        value,
    })
}
pub(super) fn random(state: &mut State<'_>) -> Result<u32, WorldTickError> {
    let mut rng = ottd_core::Randomizer::from_state(state.random_state()?);
    let value = rng.next_u32();
    for (name, word) in ["random_state[0]", "random_state[1]"]
        .into_iter()
        .zip(rng.state())
    {
        state.set_number(b"DATE", 0, name, i64::from(word))?;
    }
    Ok(value)
}
