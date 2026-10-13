use super::{
    load::{ActionResult, Session},
    load_budget::Budget,
    load_cargo::{CargoState, FileIdentity},
    load_cargo_translation::Table,
    load_engine_mapping::Kind,
    load_specs::{RoadRequest, Specs},
    load_types::LoadLocation,
    records::Reader,
};

#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    pub identity: FileIdentity,
    pub version: u8,
    pub table: &'a Table,
    pub state: &'a CargoState,
}

impl Specs {
    pub(super) fn road_cargo(
        &mut self,
        input: Input<'_>,
        request: RoadRequest,
        reader: &mut Reader<'_>,
        budget: &mut Budget,
        location: LoadLocation,
        mut resolved: impl FnMut(&Self, usize, usize) -> ActionResult,
    ) -> ActionResult {
        for local in request.first..request.first.saturating_add(request.count) {
            let local = u16::try_from(local & u32::from(u16::MAX))
                .map_err(|_| Session::unsupported(location, 0, "cargo road local"))?;
            budget.payload(
                std::mem::size_of::<super::load_specs::Owner>()
                    .saturating_add(std::mem::size_of::<super::load_specs::Temporary>())
                    .saturating_add(32),
                location,
            )?;
            let index = self
                .acquire(input.identity.grfid, Kind::Road, local, false, location)?
                .ok_or_else(|| Session::unsupported(location, 0, "cargo road allocation"))?;
            resolved(self, index, reader.remaining())?;
            let temporary = self
                .temporary
                .get_mut(index)
                .ok_or_else(|| Session::unsupported(location, 0, "cargo road temporary"))?;
            temporary.defaultcargo_grfid = Some(input.identity.grfid);
            temporary.defaultcargo_file = Some(input.identity);
            let value = reader.byte()?;
            let owner = self
                .owners
                .get_mut(index)
                .ok_or_else(|| Session::unsupported(location, 0, "cargo road owner"))?;
            owner.spec.info.cargo_type = if value == u8::MAX {
                u8::MAX
            } else {
                input
                    .table
                    .translate(value, false, input.version, input.state)
            };
            owner.spec.info.cargo_label = crate::content::CargoLabelSource::Fixed(u32::MAX);
        }
        Ok(())
    }
}
