use super::{
    language_pack::{Pack, PackError},
    load::Session,
    load_budget::Budget,
    load_language::LanguageMap,
    load_types::{ControlLoadError, LoadLocation, LoadStage},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) struct LanguageLimits {
    pub packs: usize,
    pub source_bytes: usize,
    pub map_pairs: usize,
}
impl Default for LanguageLimits {
    fn default() -> Self {
        Self {
            packs: 256,
            source_bytes: 64 << 20,
            map_pairs: 1_000_000,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct LanguageInput<'a> {
    pub packs: &'a [&'a [u8]],
    pub selected: u8,
    pub limits: LanguageLimits,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct FileLanguage {
    pub name: String,
    pub grfid: u32,
    pub features: u32,
    pub maps: BTreeMap<u32, LanguageMap>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct LanguageSnapshot {
    pub stage: u8,
    pub file: usize,
    pub line: u32,
    pub offset: usize,
    pub files: Vec<FileLanguage>,
}
#[derive(Debug, serde::Serialize)]
pub(super) struct LanguageReport {
    pub catalog: Vec<Pack>,
    pub selected: u8,
    pub admissions: Vec<&'static str>,
    pub events: Vec<LanguageSnapshot>,
}
pub(super) struct LanguageState {
    pub report: LanguageReport,
    pairs: usize,
    limits: LanguageLimits,
}
impl LanguageState {
    pub(super) fn new(
        input: LanguageInput<'_>,
        location: LoadLocation,
        budget: &mut Budget,
    ) -> Result<Self, ControlLoadError> {
        let refused = |resource| ControlLoadError::ResourceLimit { location, resource };
        if input.packs.len() > input.limits.packs {
            return Err(refused("language packs"));
        }
        budget.payload(std::mem::size_of::<LanguageReport>(), location)?;
        let mut report = LanguageReport {
            catalog: Vec::new(),
            selected: input.selected,
            admissions: Vec::new(),
            events: Vec::new(),
        };
        let mut source_bytes = 0_usize;
        let mut selected = false;
        for bytes in input.packs {
            source_bytes = source_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| refused("language source bytes"))?;
            if source_bytes > input.limits.source_bytes {
                return Err(refused("language source bytes"));
            }
            budget.payload(std::mem::size_of::<&str>(), location)?;
            let header = match Pack::header(bytes) {
                Ok(header) => header,
                Err(PackError::Header) => {
                    report.admissions.push("invalid");
                    continue;
                }
                Err(PackError::Body | PackError::NameSlot) => {
                    return Err(ControlLoadError::InvalidNativeDomain {
                        location,
                        detail: "language header parser domain",
                    });
                }
            };
            if report
                .catalog
                .iter()
                .any(|p: &Pack| p.language == header.language)
            {
                report.admissions.push("duplicate");
                continue;
            }
            header
                .admit_names()
                .map_err(|_| ControlLoadError::InvalidNativeDomain {
                    location,
                    detail: "unterminated language name slot",
                })?;
            if header.language == input.selected {
                header
                    .body(bytes)
                    .map_err(|_| ControlLoadError::InvalidNativeDomain {
                        location,
                        detail: "invalid selected language body",
                    })?;
                selected = true;
            }
            report.admissions.push("accepted");
            budget.payload(std::mem::size_of::<Pack>(), location)?;
            report.catalog.push(header);
        }
        if !selected {
            return Err(ControlLoadError::InvalidNativeDomain {
                location,
                detail: "selected language unavailable",
            });
        }
        Ok(Self {
            report,
            pairs: 0,
            limits: input.limits,
        })
    }
    pub(super) const fn pair(&mut self, location: LoadLocation) -> Result<(), ControlLoadError> {
        self.pairs = self.pairs.saturating_add(1);
        if self.pairs > self.limits.map_pairs {
            return Err(ControlLoadError::ResourceLimit {
                location,
                resource: "language mapping pairs",
            });
        }
        Ok(())
    }
}

impl Session<'_, '_> {
    pub(super) fn language_snapshot(
        &mut self,
        location: LoadLocation,
    ) -> Result<(), ControlLoadError> {
        let Some(state) = self.language.as_mut() else {
            return Ok(());
        };
        let bytes = self
            .registry
            .files
            .iter()
            .fold(0_usize, |size, file| {
                size.saturating_add(std::mem::size_of::<FileLanguage>())
                    .saturating_add(file.name.len())
                    .saturating_add(file.language_maps.values().fold(0_usize, |size, map| {
                        size.saturating_add(std::mem::size_of::<(u32, LanguageMap)>())
                            .saturating_add(
                                map.genders
                                    .len()
                                    .saturating_add(map.cases.len())
                                    .saturating_mul(2),
                            )
                    }))
            })
            .saturating_add(std::mem::size_of::<LanguageSnapshot>());
        self.budget.trace(bytes, location)?;
        let phase = match location.stage {
            LoadStage::FileScan => 0,
            LoadStage::SafetyScan => 1,
            LoadStage::LabelScan => 2,
            LoadStage::Init => 3,
            LoadStage::Reserve => 4,
            LoadStage::Activation => 5,
        };
        state.report.events.push(LanguageSnapshot {
            stage: phase,
            file: location.file,
            line: location.line,
            offset: location.offset,
            files: self
                .registry
                .files
                .iter()
                .map(|file| FileLanguage {
                    name: file.name.to_owned(),
                    grfid: file.grfid,
                    features: file.features,
                    maps: file.language_maps.clone(),
                })
                .collect(),
        });
        Ok(())
    }
}
