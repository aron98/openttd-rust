use super::{
    language_pack::{BuiltinPack, Pack, PackError},
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo: Option<super::load_cargo_translation::Table>,
    pub name: String,
    pub grfid: u32,
    pub features: u32,
    pub maps: BTreeMap<u32, LanguageMap>,
}
#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct LanguageSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo: Option<super::load_cargo::CargoState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specs: Option<super::load_specs::Specs>,
    pub stage: u8,
    pub file: usize,
    pub line: u32,
    pub offset: usize,
    pub files: Vec<FileLanguage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub strings: Vec<super::load_strings::Entry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub translation_errors: Vec<super::load_string_actions::TranslationFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<super::load_currency::CurrencyState>,
}
#[derive(Debug, serde::Serialize)]
pub(super) struct LanguageReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo: Option<super::load_cargo::CargoState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specs: Option<super::load_specs::Specs>,
    pub catalog: Vec<Pack>,
    pub selected: u8,
    pub admissions: Vec<&'static str>,
    pub events: Vec<LanguageSnapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub strings: Vec<super::load_strings::Entry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub translation_errors: Vec<super::load_string_actions::TranslationFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<super::load_currency::CurrencyState>,
}
pub(super) struct LanguageState<'a> {
    pub report: LanguageReport,
    pub builtins: BuiltinPack<'a>,
    pairs: usize,
    limits: LanguageLimits,
}
impl<'a> LanguageState<'a> {
    pub(super) fn new(
        input: LanguageInput<'a>,
        location: LoadLocation,
        budget: &mut Budget,
    ) -> Result<Self, ControlLoadError> {
        let refused = |resource| ControlLoadError::ResourceLimit { location, resource };
        if input.packs.len() > input.limits.packs {
            return Err(refused("language packs"));
        }
        budget.payload(std::mem::size_of::<LanguageReport>(), location)?;
        let mut report = LanguageReport {
            cargo: None,
            catalog: Vec::new(),
            selected: input.selected,
            admissions: Vec::new(),
            events: Vec::new(),
            strings: Vec::new(),
            translation_errors: Vec::new(),
            currency: None,
            specs: None,
        };
        let mut source_bytes = 0_usize;
        let mut selected = None;
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
                selected = Some(BuiltinPack::new(bytes).map_err(|_| {
                    ControlLoadError::InvalidNativeDomain {
                        location,
                        detail: "invalid selected language body",
                    }
                })?);
            }
            report.admissions.push("accepted");
            budget.payload(std::mem::size_of::<Pack>(), location)?;
            report.catalog.push(header);
        }
        let builtins = selected.ok_or(ControlLoadError::InvalidNativeDomain {
            location,
            detail: "selected language unavailable",
        })?;
        Ok(Self {
            report,
            builtins,
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
                    .saturating_add(
                        file.cargo
                            .as_ref()
                            .map_or(0, super::load_cargo_translation::Table::snapshot_bytes),
                    )
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
            .saturating_add(std::mem::size_of::<LanguageSnapshot>())
            .saturating_add(self.strings.snapshot_bytes())
            .saturating_add(self.string_errors.iter().fold(0_usize, |bytes, error| {
                bytes
                    .saturating_add(std::mem::size_of::<
                        super::load_string_actions::TranslationFailure,
                    >())
                    .saturating_add(error.data.len())
                    .saturating_add(error.custom_message.len())
            }));
        let bytes = bytes.saturating_add(
            self.currency
                .as_ref()
                .map_or(0, super::load_currency::CurrencyState::snapshot_bytes),
        );
        let bytes = bytes.saturating_add(
            self.specs
                .as_ref()
                .map_or(0, super::load_specs::Specs::snapshot_bytes),
        );
        let bytes = bytes.saturating_add(
            self.cargo
                .as_ref()
                .map_or(0, super::load_cargo::CargoState::snapshot_bytes),
        );
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
            cargo: self.cargo.clone(),
            specs: self.specs.clone(),
            stage: phase,
            file: location.file,
            line: location.line,
            offset: location.offset,
            strings: self.strings.entries.clone(),
            translation_errors: self.string_errors.clone(),
            currency: self.currency.clone(),
            files: self
                .registry
                .files
                .iter()
                .map(|file| FileLanguage {
                    cargo: file.cargo.clone(),
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
