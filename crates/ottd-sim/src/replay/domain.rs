use super::{ReplayError, runtime};
use crate::world_access::{field, unsigned};
use ottd_save::{WireValue, world::World};
pub(super) fn validate(world: &World) -> Result<(), ReplayError> {
    runtime::observe(world)?;
    for id in [b"NGRF", b"LGRJ"] {
        if world
            .tables()
            .get(id)
            .is_none_or(|t| !t.records().is_empty())
        {
            return Err(ReplayError::Unsupported("active content or linkgraph jobs"));
        }
    }
    if !matches!(field(world,b"LGRS",0,"running")?,WireValue::Array(v) if v.is_empty()) {
        return Err(ReplayError::Unsupported("running linkgraph jobs"));
    }
    if unsigned(world, b"DATE", 0, "pause_mode")? & 0b0101_0100 != 0 {
        return Err(ReplayError::Unsupported(
            "network or linkgraph pause needs native load handling",
        ));
    }
    if world
        .tables()
        .get(b"CITY")
        .is_none_or(|t| t.records().is_empty())
    {
        return Err(ReplayError::Unsupported(
            "native normal game requires a town",
        ));
    }
    let companies = world
        .tables()
        .get(b"PLYR")
        .ok_or(ReplayError::Unsupported("company pool"))?;
    if companies.records().is_empty() {
        return Err(ReplayError::Unsupported(
            "native load would create a company",
        ));
    }
    for id in companies.records().keys() {
        if unsigned(world, b"PLYR", *id, "is_ai")? != 0 {
            return Err(ReplayError::Unsupported("active AI script"));
        }
    }
    let gs = world
        .tables()
        .get(b"GSDT")
        .and_then(|t| t.records().get(&0))
        .ok_or(ReplayError::Unsupported("GameScript record"))?;
    if gs.tail() != [0]
        || !matches!(field(world,b"GSDT",0,"name")?,WireValue::Bytes(v) if v.is_empty())
    {
        return Err(ReplayError::Unsupported("active GameScript"));
    }
    Ok(())
}
