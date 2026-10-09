use super::{text::Budget, types::ScanError};
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Default, Clone, Copy)]
enum Destination {
    #[default]
    Outer,
    Case(u8),
    PluralGender(u8),
}

#[derive(Default)]
pub(super) struct Choices {
    pub output: Vec<u8>,
    case: Option<BTreeMap<u8, Vec<u8>>>,
    plural_gender: Option<BTreeMap<u8, Vec<u8>>>,
    case_destination: Option<u8>,
    destination: Destination,
}
impl Choices {
    pub(super) fn append(
        &mut self,
        bytes: &[u8],
        budget: &mut Budget,
        offset: usize,
    ) -> Result<(), ScanError> {
        budget.emit(bytes.len(), offset)?;
        let target = match self.destination {
            Destination::Outer => Some(&mut self.output),
            Destination::Case(index) => self.case.as_mut().and_then(|map| map.get_mut(&index)),
            Destination::PluralGender(index) => self
                .plural_gender
                .as_mut()
                .and_then(|map| map.get_mut(&index)),
        };
        if let Some(target) = target {
            target.extend_from_slice(bytes);
        }
        Ok(())
    }
    pub(super) fn start(&mut self, case: bool) {
        if self.plural_gender.is_some() || (case && self.case.is_some()) {
            return;
        }
        if case {
            self.case = Some(BTreeMap::new());
        } else {
            self.plural_gender = Some(BTreeMap::new());
        }
    }
    pub(super) fn next(&mut self, index: u8) {
        let is_plural = self.plural_gender.is_some();
        let target = if is_plural {
            &mut self.plural_gender
        } else {
            &mut self.case
        };
        if let Some(map) = target {
            if let Entry::Vacant(entry) = map.entry(index) {
                entry.insert(Vec::new());
                self.destination = if is_plural {
                    Destination::PluralGender(index)
                } else {
                    self.case_destination = Some(index);
                    Destination::Case(index)
                };
            }
        }
    }
    pub(super) fn finish(&mut self, budget: &mut Budget, offset: usize) -> Result<(), ScanError> {
        let map = if let Some(map) = self.plural_gender.take() {
            self.destination = self
                .case_destination
                .map_or(Destination::Outer, Destination::Case);
            map
        } else if let Some(map) = self.case.take() {
            self.case_destination = None;
            self.destination = Destination::Outer;
            map
        } else {
            return Ok(());
        };
        if let Some(bytes) = map.get(&0) {
            self.append(bytes, budget, offset)?;
        }
        Ok(())
    }
}
