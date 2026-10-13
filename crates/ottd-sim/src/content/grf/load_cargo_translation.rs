use super::{
    load::ActionResult,
    load_budget::Budget,
    load_cargo::{CargoState, Change},
    load_types::LoadLocation,
    records::Reader,
};
use serde::Serialize;

pub(super) fn apply_translation(
    registry: &mut super::load_registry::Registry<'_>,
    overrides: &std::collections::BTreeMap<u32, u32>,
    request: Request,
    reader: &mut Reader<'_>,
    budget: &mut Budget,
    location: LoadLocation,
) -> ActionResult<Change> {
    let file = registry
        .file_mut(location.file)
        .ok_or_else(|| super::load::Session::unsupported(location, 0, "cargo file"))?;
    let grfid = file.grfid;
    let table = file
        .cargo
        .as_mut()
        .ok_or_else(|| super::load::Session::unsupported(location, 0, "cargo table"))?;
    let result = table.property(request, reader, budget, location)?;
    if result != Change::Success {
        return Ok(result);
    }
    let target = overrides
        .get(&grfid)
        .and_then(|target| registry.files.iter().position(|file| file.grfid == *target));
    if let Some(target) = target {
        let list = &registry
            .file(location.file)
            .and_then(|file| file.cargo.as_ref())
            .ok_or_else(|| super::load::Session::unsupported(location, 0, "cargo source table"))?
            .cargo_list;
        budget.payload(list.len().saturating_mul(4), location)?;
        let list = list.clone();
        if let Some(table) = registry
            .files
            .get_mut(target)
            .and_then(|file| file.cargo.as_mut())
        {
            table.cargo_list = list;
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Table {
    pub cargo_list: Vec<u32>,
    pub fallback: bool,
    pub cargo_map: Vec<u8>,
}
impl Default for Table {
    fn default() -> Self {
        Self {
            cargo_list: Vec::new(),
            fallback: false,
            cargo_map: vec![0; 64],
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct Request {
    pub first: u16,
    pub count: u16,
    pub property: u8,
}
impl Table {
    pub(super) fn snapshot_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.cargo_list.len().saturating_mul(4))
            .saturating_add(self.cargo_map.len())
    }

    pub(super) fn selected<'a>(&'a self, version: u8, state: &'a CargoState) -> &'a [u32] {
        if !self.cargo_list.is_empty() {
            &self.cargo_list
        } else if version < 7 {
            &state.climate_dependent
        } else {
            &state.climate_independent
        }
    }

    pub(super) fn translate(&self, cargo: u8, usebit: bool, version: u8, state: &CargoState) -> u8 {
        let labels: &[u32] = if version < 7 && !usebit && !self.fallback {
            &state.climate_dependent
        } else if self.cargo_list.is_empty() {
            &state.climate_independent
        } else {
            &self.cargo_list
        };
        labels
            .get(usize::from(cargo))
            .and_then(|label| state.label_map.get(label))
            .copied()
            .unwrap_or(u8::MAX)
    }

    pub(super) fn rebuild_inverse(&mut self, version: u8, state: &CargoState) {
        for (id, owner) in state.owners.iter().enumerate() {
            let translated = if owner.spec.bitnum == u8::MAX {
                u8::MAX
            } else {
                self.selected(version, state)
                    .iter()
                    .position(|&label| label == owner.spec.label)
                    .and_then(|index| u8::try_from(index).ok())
                    .unwrap_or(u8::MAX)
            };
            if let Some(entry) = self.cargo_map.get_mut(id) {
                *entry = translated;
            }
        }
    }

    pub(super) fn fallback_label(
        &mut self,
        slots: std::ops::Range<u16>,
        label: u32,
        version: u8,
        state: &CargoState,
        budget: &mut Budget,
        location: LoadLocation,
    ) -> ActionResult {
        if self.cargo_list.is_empty() {
            budget.payload(
                self.selected(version, state).len().saturating_mul(4),
                location,
            )?;
            self.cargo_list = self.selected(version, state).to_vec();
            self.fallback = true;
        }
        if self.fallback {
            let last = usize::from(slots.end);
            if self.cargo_list.len() < last {
                budget.payload(
                    last.saturating_sub(self.cargo_list.len()).saturating_mul(4),
                    location,
                )?;
                self.cargo_list.resize(last, u32::MAX);
            }
            *self
                .cargo_list
                .get_mut(usize::from(slots.start))
                .ok_or_else(|| {
                    super::load::Session::unsupported(location, 0, "cargo fallback slot")
                })? = label;
        }
        Ok(())
    }

    pub(super) fn property(
        &mut self,
        request: Request,
        reader: &mut Reader<'_>,
        budget: &mut Budget,
        location: LoadLocation,
    ) -> ActionResult<Change> {
        self.fallback = false;
        if request.first != 0 {
            return Ok(Change::InvalidId);
        }
        self.cargo_list.clear();
        budget.payload(usize::from(request.count).saturating_mul(4), location)?;
        self.cargo_list.reserve(usize::from(request.count));
        for _ in 0..request.count {
            self.cargo_list.push(reader.dword()?.swap_bytes());
        }
        Ok(Change::Success)
    }
}
