// SPDX-License-Identifier: GPL-2.0-only
// OpenTTD 14ec60f: currency.cpp and newgrf_act0_globalvar.cpp property 0A.
use super::{
    load_currency_data::DEFAULTS, load_string_mapping::map_string, load_strings::StringTable,
};

pub(super) struct CurrencyDefault {
    pub rate: u16,
    pub separator: &'static str,
    pub to_euro: i32,
    pub prefix: &'static str,
    pub suffix: &'static str,
    pub code: &'static str,
    pub symbol_pos: u8,
    pub name: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct CurrencyOwner {
    pub rate: u16,
    pub separator: String,
    pub to_euro: i32,
    pub prefix: Vec<u8>,
    pub suffix: Vec<u8>,
    pub code: String,
    pub symbol_pos: u8,
    pub name: u32,
}

impl CurrencyOwner {
    pub(super) fn snapshot_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.separator.len())
            .saturating_add(self.prefix.len())
            .saturating_add(self.suffix.len())
            .saturating_add(self.code.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct CurrencyOwners {
    pub entries: Vec<CurrencyOwner>,
}
impl Default for CurrencyOwners {
    fn default() -> Self {
        Self::reset(None)
    }
}
impl CurrencyOwners {
    pub(super) fn initial_bytes() -> usize {
        DEFAULTS.iter().fold(0_usize, |size, spec| {
            size.saturating_add(std::mem::size_of::<CurrencyOwner>())
                .saturating_add(spec.separator.len())
                .saturating_add(spec.prefix.len())
                .saturating_add(spec.suffix.len())
                .saturating_add(spec.code.len())
        })
    }

    pub(super) fn reset(custom: Option<&CurrencyOwner>) -> Self {
        Self {
            entries: DEFAULTS
                .iter()
                .enumerate()
                .map(|(index, spec)| {
                    if index == 31 {
                        if let Some(owner) = custom {
                            return owner.clone();
                        }
                    }
                    CurrencyOwner {
                        rate: spec.rate,
                        separator: spec.separator.to_owned(),
                        to_euro: spec.to_euro,
                        prefix: spec.prefix.as_bytes().to_vec(),
                        suffix: spec.suffix.as_bytes().to_vec(),
                        code: spec.code.to_owned(),
                        symbol_pos: spec.symbol_pos,
                        name: spec.name,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(transparent)]
pub(super) struct CurrencyIndex(u8);

impl CurrencyIndex {
    pub(super) fn from_grf(index: u32) -> Self {
        const CONVERSION: [u8; 19] = [
            0, 1, 12, 8, 3, 10, 14, 19, 4, 5, 9, 11, 13, 6, 17, 16, 23, 21, 2,
        ];
        let narrowed = index.to_le_bytes()[0];
        Self(
            CONVERSION
                .get(usize::from(narrowed))
                .copied()
                .unwrap_or(narrowed),
        )
    }

    pub(super) fn offset(self) -> usize {
        usize::from(self.0)
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(super) struct PendingCurrencyName {
    pub grfid: u32,
    pub source: u16,
    pub currency: CurrencyIndex,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub(super) struct CurrencyState {
    pub owners: CurrencyOwners,
    pub pending: Vec<PendingCurrencyName>,
}
impl CurrencyState {
    pub(super) fn with_custom(owner: &CurrencyOwner) -> Self {
        Self {
            owners: CurrencyOwners::reset(Some(owner)),
            pending: Vec::new(),
        }
    }

    pub(super) fn queue(&mut self, grfid: u32, index: u32, source: u16) {
        let currency = CurrencyIndex::from_grf(index);
        if let Some(owner) = self.owners.entries.get_mut(currency.offset()) {
            owner.name = 2;
            owner.code.clear();
            self.pending.push(PendingCurrencyName {
                grfid,
                source,
                currency,
            });
        }
    }

    pub(super) fn finalize(&mut self, strings: &StringTable) {
        for assignment in &self.pending {
            if let Some(owner) = self
                .owners
                .entries
                .get_mut(usize::from(assignment.currency.0))
            {
                owner.name = map_string(strings, assignment.grfid, assignment.source);
                owner.code.clear();
            }
        }
        self.pending.clear();
    }

    pub(super) fn snapshot_bytes(&self) -> usize {
        self.owners.entries.iter().fold(
            self.pending
                .len()
                .saturating_mul(std::mem::size_of::<PendingCurrencyName>()),
            |size, owner| size.saturating_add(owner.snapshot_bytes()),
        )
    }
}

impl super::load::Session<'_, '_> {
    pub(super) fn currency_names(
        &mut self,
        reader: &mut super::records::Reader<'_>,
        first: u32,
        items: u32,
        location: super::load_types::LoadLocation,
    ) -> super::load::ActionResult {
        for index in first..first.saturating_add(items) {
            let source = reader.word()?;
            if location.stage == super::load_types::LoadStage::Activation {
                let grfid = self
                    .registry
                    .file(location.file)
                    .ok_or_else(|| Self::unsupported(location, 0, "missing currency dynamic file"))?
                    .grfid;
                self.budget
                    .payload(std::mem::size_of::<PendingCurrencyName>(), location)?;
                self.currency
                    .get_or_insert_with(CurrencyState::default)
                    .queue(grfid, index, source);
            }
        }
        Ok(())
    }
}
