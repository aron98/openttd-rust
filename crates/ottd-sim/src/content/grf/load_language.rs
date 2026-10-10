// SPDX-License-Identifier: GPL-2.0-only
// OpenTTD 14ec60f248547d4d062a1160f0fc26d742319888: newgrf_text.cpp, newgrf_act0_globalvar.cpp.
use super::{
    language_pack::PLURAL_RULES,
    load::{ActionResult, Session},
    load_types::{LoadLocation, LoadStage},
    records::Reader,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub(super) struct LanguageMap {
    pub genders: Vec<(u8, u8)>,
    pub cases: Vec<(u8, u8)>,
    pub plural: u8,
}

impl Session<'_, '_> {
    pub(super) fn language_properties(
        &mut self,
        reader: &mut Reader<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let feature = reader.byte()?;
        if location.stage == LoadStage::Reserve && (feature >= 22 || feature == 14) {
            return Ok(());
        }
        let count = reader.byte()?;
        let items = u32::from(reader.byte()?);
        let first = u32::from(reader.extended()?);
        if feature >= 22 || feature == 14 {
            return Ok(());
        }
        if feature != 8 {
            return Err(Self::unsupported(location, 0, "non-language Action0 feature").into());
        }
        if location.stage == LoadStage::Activation {
            let file = self
                .registry
                .file_mut(location.file)
                .ok_or_else(|| Self::unsupported(location, 0, "missing language dynamic file"))?;
            file.features |= 1 << 8;
        }
        for _ in 0..count {
            if reader.remaining() == 0 {
                break;
            }
            let property = reader.byte()?;
            if property == 0x0a {
                self.currency_names(reader, first, items, location)?;
                continue;
            }
            if let Some(property) = super::load_currency_properties::Property::from_id(property) {
                self.currency_properties(reader, property, first, items, location)?;
                continue;
            }
            if !(0x13..=0x15).contains(&property) {
                return Err(Self::unsupported(location, 0, "unimplemented global property").into());
            }
            for language in first..first.saturating_add(items) {
                if property == 0x15 {
                    let plural = reader.byte()?;
                    let known = self.language.as_ref().is_some_and(|state| {
                        state
                            .report
                            .catalog
                            .iter()
                            .any(|pack| u32::from(pack.language) == language)
                    });
                    if location.stage == LoadStage::Activation && known && plural < PLURAL_RULES {
                        self.language_map(language, location)?.plural = plural;
                    }
                } else {
                    loop {
                        let id = reader.byte()?;
                        if id == 0 {
                            break;
                        }
                        let name = reader.string()?;
                        if location.stage != LoadStage::Activation {
                            continue;
                        }
                        let name = name.strip_prefix(&[0xc3, 0x9e]).unwrap_or(name);
                        let native = self
                            .language
                            .as_ref()
                            .and_then(|state| {
                                state
                                    .report
                                    .catalog
                                    .iter()
                                    .find(|pack| u32::from(pack.language) == language)
                            })
                            .and_then(|pack| pack.name_index(name, property == 0x13));
                        if let Some(native) = native {
                            if let Some(state) = self.language.as_mut() {
                                state.pair(location)?;
                            }
                            self.budget.payload(2, location)?;
                            let map = self.language_map(language, location)?;
                            if property == 0x13 {
                                map.genders.push((id, native));
                            } else {
                                map.cases.push((id, native));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn language_map(
        &mut self,
        language: u32,
        location: LoadLocation,
    ) -> ActionResult<&mut LanguageMap> {
        let file = self
            .registry
            .file_mut(location.file)
            .ok_or_else(|| Self::unsupported(location, 0, "missing language dynamic file"))?;
        if !file.language_maps.contains_key(&language) {
            self.budget
                .payload(std::mem::size_of::<(u32, LanguageMap)>(), location)?;
        }
        Ok(file.language_maps.entry(language).or_default())
    }
}
impl LanguageMap {
    pub(super) fn forward(&self, id: u8, gender: bool) -> Option<u8> {
        let pairs = if gender { &self.genders } else { &self.cases };
        pairs
            .iter()
            .find(|&&(source, _)| source == id)
            .map(|&(_, native)| native)
    }
    pub(super) fn reverse(&self, id: u8, gender: bool) -> Option<u8> {
        let pairs = if gender { &self.genders } else { &self.cases };
        pairs
            .iter()
            .find(|&&(_, native)| native == id)
            .map(|&(source, _)| source)
    }
}
