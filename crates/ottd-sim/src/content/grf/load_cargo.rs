use super::{
    load::ActionResult,
    load_budget::Budget,
    load_types::{ControlLoadError, LoadLocation},
    records::Reader,
};
use crate::content::{CargoSpec, Climate};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct FileIdentity {
    pub index: usize,
    pub grfid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Owner {
    #[serde(flatten)]
    pub spec: CargoSpec,
    pub name: u32,
    pub name_single: u32,
    pub units_volume: u32,
    pub quantifier: u32,
    pub abbrev: u32,
    pub sprite: u32,
    pub file: Option<FileIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CargoState {
    pub owners: Vec<Owner>,
    pub cargo_mask: u64,
    pub standard_cargo_mask: u64,
    pub label_map: BTreeMap<u32, u8>,
    pub default_labels: Vec<u32>,
    pub climate_dependent: [u32; 12],
    pub climate_independent: [u32; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    Success,
    Unhandled,
    Unknown,
    InvalidId,
}

#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub identity: FileIdentity,
    pub version: u8,
}

impl CargoState {
    pub(super) fn new(
        climate: Climate,
        inherited_standard_mask: u64,
        budget: &mut Budget,
        location: LoadLocation,
    ) -> Result<Self, ControlLoadError> {
        budget.payload(
            std::mem::size_of::<Self>().saturating_add(
                64_usize.saturating_mul(std::mem::size_of::<Owner>().saturating_add(32)),
            ),
            location,
        )?;
        let mut empty = CargoSpec::EMPTY;
        empty.town_production_effect = 4;
        let empty = Owner {
            spec: empty,
            name: 0,
            name_single: 0,
            units_volume: 0,
            quantifier: 0,
            abbrev: 0,
            sprite: 0,
            file: None,
        };
        let mut state = Self {
            owners: vec![empty; 64],
            cargo_mask: 0,
            standard_cargo_mask: inherited_standard_mask,
            label_map: BTreeMap::new(),
            default_labels: Vec::new(),
            climate_dependent: [u32::MAX; 12],
            climate_independent: [u32::MAX; 32],
        };
        let indices = crate::content::cargo_data::CLIMATE_CARGO
            .get(usize::from(climate as u8))
            .ok_or(ControlLoadError::InvalidNativeDomain {
                location,
                detail: "cargo climate",
            })?;
        for (id, &index) in indices.iter().enumerate() {
            let mut spec = *crate::content::cargo_data::CARGO.get(index).ok_or(
                ControlLoadError::InvalidNativeDomain {
                    location,
                    detail: "cargo template",
                },
            )?;
            spec.town_production_effect = 4;
            let &[name, name_single, units_volume, quantifier, abbrev, sprite] =
                super::load_cargo_data::PRESENTATION.get(index).ok_or(
                    ControlLoadError::InvalidNativeDomain {
                        location,
                        detail: "cargo presentation",
                    },
                )?;
            let owner = state
                .owners
                .get_mut(id)
                .ok_or(ControlLoadError::InvalidNativeDomain {
                    location,
                    detail: "cargo slot",
                })?;
            *owner = Owner {
                spec,
                name,
                name_single,
                units_volume,
                quantifier,
                abbrev,
                sprite,
                file: None,
            };
            if spec.bitnum != u8::MAX {
                state.cargo_mask |= 1_u64 << id;
                state.default_labels.push(spec.label);
                *state.climate_dependent.get_mut(id).ok_or(
                    ControlLoadError::InvalidNativeDomain {
                        location,
                        detail: "climate slot",
                    },
                )? = spec.label;
                *state
                    .climate_independent
                    .get_mut(usize::from(spec.bitnum))
                    .ok_or(ControlLoadError::InvalidNativeDomain {
                        location,
                        detail: "climate bit",
                    })? = spec.label;
            }
        }
        state.rebuild_labels();
        Ok(state)
    }

    pub(super) fn snapshot_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.owners
                    .len()
                    .saturating_mul(std::mem::size_of::<Owner>()),
            )
            .saturating_add(self.label_map.len().saturating_mul(32))
            .saturating_add(self.default_labels.len().saturating_mul(4))
    }

    pub(super) fn vanilla(
        climate: Climate,
        budget: &mut Budget,
        location: LoadLocation,
    ) -> Result<Self, ControlLoadError> {
        let mut state = Self::new(climate, 0, budget, location)?;
        state.standard_cargo_mask = state.cargo_mask;
        Ok(state)
    }

    fn rebuild_labels(&mut self) {
        self.label_map.clear();
        for (id, owner) in (0_u8..64).zip(&self.owners) {
            if owner.spec.bitnum != u8::MAX && owner.spec.label != 0 && owner.spec.label != u32::MAX
            {
                self.label_map.entry(owner.spec.label).or_insert(id);
            }
        }
    }

    pub(super) fn property(
        &mut self,
        table: &mut super::load_cargo_translation::Table,
        binding: Binding,
        request: super::load_cargo_translation::Request,
        reader: &mut Reader<'_>,
        budget: &mut Budget,
        location: LoadLocation,
    ) -> ActionResult<Change> {
        if location.stage == super::LoadStage::Activation {
            return Ok(Change::Unhandled);
        }
        let last = request.first.saturating_add(request.count);
        if last > 64 {
            return Ok(Change::InvalidId);
        }
        if !matches!(request.property, 1 | 8 | 23) {
            return Err(super::load::Session::unsupported(
                location,
                0,
                "unimplemented cargo property",
            )
            .into());
        }
        let mut result = Change::Success;
        for id in request.first..last {
            let owner = self
                .owners
                .get_mut(usize::from(id))
                .ok_or_else(|| super::load::Session::unsupported(location, 0, "cargo owner"))?;
            match request.property {
                8 => {
                    let value = reader.byte()?;
                    owner.spec.bitnum = value;
                    if value == u8::MAX {
                        self.cargo_mask &= !(1_u64 << id);
                    } else {
                        owner.file = Some(binding.identity);
                        self.cargo_mask |= 1_u64 << id;
                    }
                    budget.payload(64_usize.saturating_mul(32), location)?;
                    self.rebuild_labels();
                }
                23 => {
                    let label = reader.dword()?.swap_bytes();
                    owner.spec.label = label;
                    budget.payload(64_usize.saturating_mul(32), location)?;
                    self.rebuild_labels();
                    table.fallback_label(
                        id..last,
                        label,
                        binding.version,
                        self,
                        budget,
                        location,
                    )?;
                }
                1 => result = Change::Unknown,
                _ => {
                    return Err(
                        super::load::Session::unsupported(location, 0, "cargo dispatch").into(),
                    );
                }
            }
        }
        Ok(result)
    }
}
