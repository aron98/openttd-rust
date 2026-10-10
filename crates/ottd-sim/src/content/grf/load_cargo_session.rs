use super::{
    load::{ActionResult, Session},
    load_cargo::{CargoState, Change, FileIdentity},
    load_cargo_translation::{Request, Table},
    load_specs::Specs,
    load_types::{ControlLoadError, LoadFailure, LoadLocation, LoadStage},
    records::Reader,
};

impl Session<'_, '_> {
    pub(super) fn ensure_cargo(&mut self, location: LoadLocation) -> ActionResult {
        if self.cargo.is_none() {
            let climate = self
                .environment
                .as_ref()
                .map_or(crate::content::Climate::Temperate, |environment| {
                    environment.settings.climate
                });
            self.cargo = Some(CargoState::vanilla(climate, &mut self.budget, location)?);
        }
        for file in &mut self.registry.files {
            if file.cargo.is_none() {
                self.budget
                    .payload(std::mem::size_of::<Table>().saturating_add(64), location)?;
                file.cargo = Some(Table::default());
            }
        }
        if self.specs.is_none() {
            let dynamic = self
                .environment
                .as_ref()
                .is_none_or(|environment| environment.settings.patch.dynamic_engines);
            self.budget.payload(
                256_usize.saturating_mul(
                    std::mem::size_of::<super::load_specs::Owner>()
                        .saturating_add(std::mem::size_of::<super::load_specs::Temporary>())
                        .saturating_add(32),
                ),
                location,
            )?;
            let mut specs = Specs::new(dynamic, location)?;
            specs.install_reserve_defaults();
            self.specs = Some(specs);
        }
        Ok(())
    }

    fn cargo_request(
        first: u32,
        count: u32,
        property: u8,
        location: LoadLocation,
    ) -> Result<Request, ControlLoadError> {
        Ok(Request {
            first: u16::try_from(first)
                .map_err(|_| Self::unsupported(location, 0, "cargo first width"))?,
            count: u16::try_from(count)
                .map_err(|_| Self::unsupported(location, 0, "cargo count width"))?,
            property,
        })
    }

    fn cargo_result(&mut self, result: Change, location: LoadLocation) -> ActionResult<bool> {
        let failure = match result {
            Change::Success | Change::Unhandled => return Ok(false),
            Change::Unknown => LoadFailure::UnknownProperty,
            Change::InvalidId => LoadFailure::InvalidId,
        };
        self.disable(location.file, location, Some(failure))?;
        Ok(true)
    }

    pub(super) fn cargo_properties(
        &mut self,
        reader: &mut Reader<'_>,
        first: u32,
        count: u32,
        properties: u8,
        location: LoadLocation,
    ) -> ActionResult {
        if location.stage == LoadStage::Activation {
            return Ok(());
        }
        self.ensure_cargo(location)?;
        for _ in 0..properties {
            if reader.remaining() == 0 {
                break;
            }
            let property = reader.byte()?;
            let index = self
                .registry
                .file_index(location.file)
                .ok_or_else(|| Self::unsupported(location, 0, "cargo file"))?;
            let file = self
                .registry
                .file_mut(location.file)
                .ok_or_else(|| Self::unsupported(location, 0, "cargo file"))?;
            let identity = FileIdentity {
                index,
                grfid: file.grfid,
            };
            let request = Self::cargo_request(first, count, property, location)?;
            let result = self
                .cargo
                .as_mut()
                .ok_or_else(|| Self::unsupported(location, 0, "cargo state"))?
                .property(
                    file.cargo
                        .as_mut()
                        .ok_or_else(|| Self::unsupported(location, 0, "cargo table"))?,
                    super::load_cargo::Binding {
                        identity,
                        version: file.version,
                    },
                    request,
                    reader,
                    &mut self.budget,
                    location,
                )?;
            if self.cargo_result(result, location)? {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn cargo_table_property(
        &mut self,
        reader: &mut Reader<'_>,
        first: u32,
        count: u32,
        location: LoadLocation,
    ) -> ActionResult<bool> {
        self.ensure_cargo(location)?;
        let overrides = &self
            .specs
            .as_ref()
            .ok_or_else(|| Self::unsupported(location, 0, "cargo overrides"))?
            .grfid_overrides;
        let result = super::load_cargo_translation::apply_translation(
            &mut self.registry,
            overrides,
            Self::cargo_request(first, count, 9, location)?,
            reader,
            &mut self.budget,
            location,
        )?;
        self.cargo_result(result, location)
    }

    pub(super) fn finish_cargo_file(&mut self, config: usize) {
        if let Some(state) = self.cargo.as_ref() {
            if let Some(file) = self.registry.file_mut(config) {
                if let Some(table) = file.cargo.as_mut() {
                    table.rebuild_inverse(file.version, state);
                }
            }
        }
    }

    pub(super) fn road_cargo_property(
        &mut self,
        reader: &mut Reader<'_>,
        first: u32,
        count: u32,
        location: LoadLocation,
    ) -> ActionResult {
        self.ensure_cargo(location)?;
        let file = self
            .registry
            .file(location.file)
            .ok_or_else(|| Self::unsupported(location, 0, "cargo road file"))?;
        let index = self
            .registry
            .file_index(location.file)
            .ok_or_else(|| Self::unsupported(location, 0, "cargo road index"))?;
        let input = super::load_cargo_road::Input {
            identity: FileIdentity {
                index,
                grfid: file.grfid,
            },
            version: file.version,
            table: file
                .cargo
                .as_ref()
                .ok_or_else(|| Self::unsupported(location, 0, "cargo road table"))?,
            state: self
                .cargo
                .as_ref()
                .ok_or_else(|| Self::unsupported(location, 0, "cargo state"))?,
        };
        let request = super::load_specs::RoadRequest {
            grfid: file.grfid,
            first,
            count,
            property: 16,
        };
        self.specs
            .as_mut()
            .ok_or_else(|| Self::unsupported(location, 0, "cargo engine state"))?
            .road_cargo(
                input,
                request,
                reader,
                &mut self.budget,
                location,
                |_, _, _| Ok(()),
            )
    }
}
