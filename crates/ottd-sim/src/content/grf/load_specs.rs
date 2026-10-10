use super::{
    load::{ActionResult, Session},
    load_engine_mapping::{Kind, Mapping, Mappings},
    load_types::{ControlLoadError, LoadFailure, LoadLocation, LoadStage},
    records::Reader,
};
use crate::content::{EngineSpec, VehicleSpec};
use serde::Serialize;
use std::collections::BTreeMap;

const MAX_OWNERS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Dynamic {
    pub name_bytes: Vec<u8>,
    pub intro_date: i32,
    pub age: i32,
    pub flags: u8,
    pub reliability: u16,
    pub reliability_spd_dec: u16,
    pub reliability_start: u16,
    pub reliability_max: u16,
    pub reliability_final: u16,
    pub duration_phase_1: u16,
    pub duration_phase_2: u16,
    pub duration_phase_3: u16,
    pub company_avail: u16,
    pub company_hidden: u16,
    pub preview_asked: u16,
    pub preview_company: u8,
    pub preview_wait: u8,
    pub display_flags: u8,
    pub display_last_variant: u16,
}

impl Default for Dynamic {
    fn default() -> Self {
        Self {
            name_bytes: Vec::new(),
            intro_date: 0,
            age: 0,
            flags: 0,
            reliability: 0,
            reliability_spd_dec: 0,
            reliability_start: 0,
            reliability_max: 0,
            reliability_final: 0,
            duration_phase_1: 0,
            duration_phase_2: 0,
            duration_phase_3: 0,
            company_avail: 0,
            company_hidden: 0,
            preview_asked: 0,
            preview_company: u8::MAX,
            preview_wait: 0,
            display_flags: 0,
            display_last_variant: u16::MAX,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Owner {
    pub kind: Kind,
    pub spec: EngineSpec,
    pub grfid: Option<u32>,
    pub stored_grfid: u32,
    pub string_id: u16,
    pub original_image_index: u8,
    pub dynamic: Dynamic,
    pub badges: Vec<u16>,
}

impl Owner {
    fn new(mapping: &Mapping, location: LoadLocation) -> Result<Self, ControlLoadError> {
        let spec = crate::content::engines::raw_original(
            mapping.kind.index(),
            mapping.internal_id,
            mapping.engine,
        )
        .ok_or(ControlLoadError::InvalidNativeDomain {
            location,
            detail: "raw engine constructor",
        })?;
        let original = mapping.internal_id < mapping.kind.count();
        let image = match spec.vehicle {
            VehicleSpec::Rail(v) => v.image_index,
            VehicleSpec::Road(v) => v.image_index,
            VehicleSpec::Ship(v) => v.image_index,
            VehicleSpec::Aircraft(v) => v.image_index,
        };
        Ok(Self {
            kind: mapping.kind,
            spec,
            grfid: None,
            stored_grfid: 0,
            string_id: if original {
                0x8000_u16
                    .checked_add(mapping.kind.offset())
                    .and_then(|value| value.checked_add(mapping.internal_id))
                    .ok_or(ControlLoadError::InvalidNativeDomain {
                        location,
                        detail: "original engine name ID",
                    })?
            } else {
                u16::MAX
            },
            original_image_index: if original { image } else { 0 },
            dynamic: Dynamic::default(),
            badges: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(super) struct Temporary {
    pub cargo_allowed: u16,
    pub cargo_allowed_required: u16,
    pub cargo_disallowed: u16,
    pub railtypelabels: Vec<u32>,
    pub roadtramtype: u8,
    pub defaultcargo_grfid: Option<u32>,
    pub refittability: u8,
    pub rv_max_speed: u8,
    pub ctt_include_mask: u64,
    pub ctt_exclude_mask: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Specs {
    pub owners: Vec<Owner>,
    pub mappings: Mappings,
    pub temporary: Vec<Temporary>,
    pub grfid_overrides: BTreeMap<u32, u32>,
    pub dynamic_engines: bool,
    pub pool_capacity: usize,
}

impl Specs {
    pub(super) fn new(
        dynamic_engines: bool,
        location: LoadLocation,
    ) -> Result<Self, ControlLoadError> {
        let mut state = Self {
            owners: Vec::new(),
            mappings: Mappings::default(),
            temporary: Vec::new(),
            grfid_overrides: BTreeMap::new(),
            dynamic_engines,
            pool_capacity: 0,
        };
        state.reset(location)?;
        Ok(state)
    }

    pub(super) fn snapshot_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.owners
                    .len()
                    .saturating_mul(std::mem::size_of::<Owner>()),
            )
            .saturating_add(
                self.temporary
                    .len()
                    .saturating_mul(std::mem::size_of::<Temporary>().saturating_add(4)),
            )
            .saturating_add(
                self.mappings
                    .entries
                    .iter()
                    .map(|v| v.len().saturating_mul(std::mem::size_of::<Mapping>()))
                    .sum::<usize>(),
            )
            .saturating_add(self.grfid_overrides.len().saturating_mul(8))
    }

    fn install_reserve_defaults(&mut self) {
        for (source, target) in [
            (0x4444_2202_u32, 0x4444_0111_u32),
            (0x6d62_0402, 0x6d62_0401),
            (0x4d65_6f20, 0x4d65_6f17),
        ] {
            self.grfid_overrides
                .insert(source.swap_bytes(), target.swap_bytes());
        }
    }

    pub(super) fn reset(&mut self, location: LoadLocation) -> Result<(), ControlLoadError> {
        let mut owners = Vec::new();
        for mapping in self.mappings.entries.iter().flatten() {
            if owners.len() >= MAX_OWNERS {
                return Err(Self::limit(location));
            }
            owners.push(Owner::new(mapping, location)?);
        }
        owners.sort_by_key(|owner| owner.spec.id);
        self.pool_capacity = owners.len();
        self.temporary
            .resize(self.pool_capacity, Temporary::default());
        for owner in &owners {
            if let VehicleSpec::Rail(rail) = owner.spec.vehicle {
                if let Some(temp) = self.temporary.get_mut(usize::from(owner.spec.id)) {
                    temp.railtypelabels.clear();
                    for (bit, label) in [*b"RAIL", *b"ELRL", *b"MONO", *b"MGLV"]
                        .into_iter()
                        .enumerate()
                    {
                        if rail.railtypes & (1_u64 << bit) != 0 {
                            temp.railtypelabels.push(u32::from_be_bytes(label));
                        }
                    }
                }
            }
        }
        self.owners = owners;
        self.grfid_overrides.clear();
        Ok(())
    }

    const fn limit(location: LoadLocation) -> ControlLoadError {
        ControlLoadError::ResourceLimit {
            location,
            resource: "engine owners",
        }
    }

    pub(super) fn acquire(
        &mut self,
        grfid: u32,
        kind: Kind,
        local: u16,
        static_access: bool,
        location: LoadLocation,
    ) -> Result<Option<usize>, ControlLoadError> {
        let scope = if self.dynamic_engines {
            self.grfid_overrides.get(&grfid).copied().unwrap_or(grfid)
        } else {
            u32::MAX
        };
        let existing = if self.dynamic_engines {
            self.mappings.get(kind, local, scope)
        } else {
            None
        };
        let existing =
            existing.or_else(|| self.mappings.reserve(kind, local, scope, static_access));
        let engine = if let Some(engine) = existing {
            engine
        } else {
            if static_access {
                return Ok(None);
            }
            if self.owners.len() >= MAX_OWNERS {
                return Err(Self::limit(location));
            }
            let engine = u16::try_from(self.owners.len()).map_err(|_| Self::limit(location))?;
            let mapping = Mapping {
                kind,
                grfid: scope,
                internal_id: local,
                substitute_id: local.min(kind.count()),
                engine,
            };
            self.owners.push(Owner::new(&mapping, location)?);
            self.mappings.insert(mapping);
            self.pool_capacity = self.owners.len();
            self.temporary
                .resize(self.pool_capacity, Temporary::default());
            engine
        };
        let index = usize::from(engine);
        let owner = self
            .owners
            .get_mut(index)
            .ok_or(ControlLoadError::InvalidNativeDomain {
                location,
                detail: "engine mapping has no owner",
            })?;
        if owner.grfid.is_none() {
            owner.grfid = Some(grfid);
            owner.stored_grfid = grfid;
        }
        Ok(Some(index))
    }
}

#[derive(Clone, Copy)]
pub(super) struct RoadRequest {
    pub grfid: u32,
    pub first: u32,
    pub count: u32,
    pub property: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RoadResult {
    Success,
    Unknown,
    Unhandled,
}

pub(super) fn dispatch_road(
    state: Option<&mut Specs>,
    request: RoadRequest,
    reader: &mut Reader<'_>,
    budget: &mut super::load_budget::Budget,
    location: LoadLocation,
) -> ActionResult<RoadResult> {
    match location.stage {
        LoadStage::Reserve => Ok(RoadResult::Unhandled),
        LoadStage::Activation => state
            .ok_or_else(|| Session::unsupported(location, 0, "missing engine owners"))?
            .apply_road(request, reader, budget, location),
        LoadStage::FileScan | LoadStage::SafetyScan | LoadStage::LabelScan | LoadStage::Init => {
            Err(Session::unsupported(location, 0, "inactive road property stage").into())
        }
    }
}

impl Specs {
    pub(super) fn apply_road(
        &mut self,
        request: RoadRequest,
        reader: &mut Reader<'_>,
        budget: &mut super::load_budget::Budget,
        location: LoadLocation,
    ) -> ActionResult<RoadResult> {
        let RoadRequest {
            grfid,
            first,
            count,
            property,
        } = request;
        if !matches!(property, 0x01 | 0x08 | 0x09 | 0x0f | 0x11) {
            return Err(Session::unsupported(location, 0, "unimplemented road property").into());
        }
        for local in first..first.saturating_add(count) {
            let local = u16::try_from(local & u32::from(u16::MAX))
                .map_err(|_| Session::unsupported(location, 0, "road local ID"))?;
            let before = self.snapshot_bytes();
            let index = self
                .acquire(grfid, Kind::Road, local, false, location)?
                .ok_or_else(|| Session::unsupported(location, 0, "road allocation"))?;
            budget.payload(self.snapshot_bytes().saturating_sub(before), location)?;
            if property == 0x01 {
                continue;
            }
            let value = reader.byte()?;
            let owner = self
                .owners
                .get_mut(index)
                .ok_or_else(|| Session::unsupported(location, 0, "road owner index"))?;
            let VehicleSpec::Road(road) = &mut owner.spec.vehicle else {
                return Err(Session::unsupported(location, 0, "road owner type").into());
            };
            match property {
                0x08 => road.max_speed = u16::from(value),
                0x09 => road.running_cost = value,
                0x0f => road.capacity = value,
                0x11 => road.cost_factor = value,
                _ => return Err(Session::unsupported(location, 0, "road property dispatch").into()),
            }
        }
        Ok(if property == 0x01 && count != 0 {
            RoadResult::Unknown
        } else {
            RoadResult::Success
        })
    }
}

impl Session<'_, '_> {
    pub(super) fn road_properties(
        &mut self,
        reader: &mut Reader<'_>,
        first: u32,
        count: u32,
        properties: u8,
        location: LoadLocation,
    ) -> ActionResult {
        let grfid = self
            .registry
            .file(location.file)
            .ok_or_else(|| Self::unsupported(location, 0, "missing road dynamic file"))?
            .grfid;
        if self.specs.is_none() {
            let dynamic = self
                .environment
                .as_ref()
                .is_none_or(|environment| environment.settings.patch.dynamic_engines);
            let mut specs = Specs::new(dynamic, location)?;
            specs.install_reserve_defaults();
            self.budget.payload(specs.snapshot_bytes(), location)?;
            self.specs = Some(specs);
        }
        if let Some(file) = self.registry.file_mut(location.file) {
            file.features |= 1 << 1;
        }
        for _ in 0..properties {
            if reader.remaining() == 0 {
                break;
            }
            let property = reader.byte()?;
            let request = RoadRequest {
                grfid,
                first,
                count,
                property,
            };
            let state = self
                .specs
                .as_mut()
                .ok_or_else(|| Self::unsupported(location, 0, "missing engine owners"))?;
            if state.apply_road(request, reader, &mut self.budget, location)? == RoadResult::Unknown
            {
                self.disable(location.file, location, Some(LoadFailure::UnknownProperty))?;
                return Ok(());
            }
        }
        Ok(())
    }
}
