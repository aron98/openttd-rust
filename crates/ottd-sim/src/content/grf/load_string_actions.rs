use super::{
    load::{ActionResult, Session},
    load_cursor::{Cursor, Skip},
    load_strings::{Definition, StringKey, TableError},
    load_types::{ControlLoadError, LoadFailure, LoadLocation, LoadStatus},
    records::Reader,
    text_mapped::TextContext,
    text_translate::{Input, translate_in},
};

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct TranslationFailure {
    pub file: usize,
    pub line: u32,
    pub message: u32,
    pub severity: u32,
    pub parameters: [u32; 2],
    pub data: Vec<u8>,
    pub custom_message: Vec<u8>,
}

impl Session<'_, '_> {
    pub(super) fn translate_strings(
        &mut self,
        reader: &mut Reader<'_>,
        cursor: &mut Cursor<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let grfid = reader.dword()?;
        self.check_preceding(grfid, u32::MAX, location, 0x13)?;
        let Some(target) = self.registry.config_by_id(grfid, u32::MAX) else {
            return Ok(());
        };
        match self.registry.status(target) {
            LoadStatus::Unknown | LoadStatus::NotFound | LoadStatus::Disabled => return Ok(()),
            LoadStatus::Initialised => {
                self.disable(location.file, location, Some(LoadFailure::LoadAfter))?;
                cursor.skip = Skip::Stop;
                let state = self
                    .language
                    .as_ref()
                    .ok_or_else(|| Self::unsupported(location, 0x13, "missing error language"))?;
                let bytes = self
                    .strings
                    .resolve(0xb2b, state.report.selected, |id| state.builtins.lookup(id))
                    .map_err(|_| {
                        Self::unsupported(location, 0x13, "builtin error detail domain")
                    })?;
                let text = std::str::from_utf8(bytes).map_err(|_| {
                    Self::unsupported(location, 0x13, "non-UTF8 builtin error detail")
                })?;
                if text
                    .chars()
                    .any(|ch| ch < ' ' || ('\u{e000}'..='\u{e2ff}').contains(&ch))
                {
                    return Err(Self::unsupported(
                        location,
                        0x13,
                        "formatted builtin error detail",
                    )
                    .into());
                }
                self.budget.payload(bytes.len(), location)?;
                self.string_errors.push(TranslationFailure {
                    file: location.file,
                    line: location.line,
                    message: 0xb29,
                    severity: 0xb21,
                    parameters: [location.line, 0],
                    data: bytes.to_vec(),
                    custom_message: Vec::new(),
                });
                return Ok(());
            }
            LoadStatus::Activated => (),
        }
        let version = self
            .registry
            .file(location.file)
            .ok_or_else(|| Self::unsupported(location, 0x13, "missing translation file"))?
            .version;
        let language = if version >= 8 { reader.byte()? } else { 0x7f };
        let count = u16::from(reader.byte()?);
        let first = reader.word()?;
        let end = u32::from(first).saturating_add(u32::from(count));
        if !((first >= 0xd000 && end <= 0xd400) || (first >= 0xd800 && end <= 0xe000)) {
            return Ok(());
        }
        for index in 0..count {
            if reader.remaining() == 0 {
                break;
            }
            let raw = reader.string()?;
            if raw.is_empty() {
                continue;
            }
            self.add_string(
                Definition {
                    key: StringKey {
                        grfid,
                        local_id: u32::from(first).saturating_add(u32::from(index)),
                    },
                    language,
                    new_scheme: true,
                    default_id: 2,
                },
                raw,
                location,
            )?;
        }
        Ok(())
    }
    fn add_string(
        &mut self,
        request: Definition,
        raw: &[u8],
        location: LoadLocation,
    ) -> ActionResult<u32> {
        self.check_preceding(request.key.grfid, u32::MAX, location, 4)?;
        let language_state = self.language.as_ref().ok_or_else(|| {
            Self::unsupported(
                location,
                4,
                "custom strings require admitted language context",
            )
        })?;
        let pack = language_state
            .report
            .catalog
            .iter()
            .find(|pack| pack.language == language_state.report.selected)
            .ok_or_else(|| Self::unsupported(location, 4, "missing selected language pack"))?;
        let file = self.registry.file_by_id(request.key.grfid);
        let budget = &mut self.string_budget;
        self.strings
            .define(request, |strings, language| {
                let map = file.and_then(|file| file.language_maps.get(&u32::from(language)));
                translate_in(
                    Input {
                        raw,
                        newlines: true,
                        offset: location.offset,
                        context: TextContext {
                            map,
                            genders: pack.gender_count,
                            cases: pack.case_count,
                        },
                    },
                    budget,
                    |id| Some(strings.inline_id(request.key.grfid, id)),
                )
            })
            .map_err(|error| match error {
                TableError::Resource(resource)
                | TableError::Translation(super::ScanError::ResourceLimit { resource, .. }) => {
                    ControlLoadError::ResourceLimit { location, resource }.into()
                }
                TableError::Translation(_)
                | TableError::InvalidId(_)
                | TableError::DefaultCycle(_) => {
                    Self::unsupported(location, 4, "custom string translation domain").into()
                }
            })
    }

    pub(super) fn generic_names(
        &mut self,
        reader: &mut Reader<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let feature = reader.byte()?;
        if feature >= 22 && feature != 0x48 {
            return Ok(());
        }
        let language = reader.byte()?;
        let count = reader.byte()?;
        let generic = language & 0x80 != 0;
        let mut id = if generic {
            reader.word()?
        } else if feature <= 3 || feature == 21 {
            reader.extended()?
        } else {
            u16::from(reader.byte()?)
        };
        let end = id.wrapping_add(u16::from(count));
        let file = self
            .registry
            .file(location.file)
            .ok_or_else(|| Self::unsupported(location, 4, "missing custom string file"))?;
        let grfid = file.grfid;
        let new_scheme = file.version >= 7;
        while id < end && reader.remaining() != 0 {
            let raw = reader.string()?;
            if !generic && (feature <= 3 || feature == 21) {
                return Err(Self::unsupported(
                    location,
                    4,
                    "engine or badge naming owner is unimplemented",
                )
                .into());
            }
            if feature <= 3 || feature == 21 || (0xd000..0xd400).contains(&id) || id >= 0xd800 {
                self.add_string(
                    Definition {
                        key: StringKey {
                            grfid,
                            local_id: u32::from(id),
                        },
                        language: language & 0x7f,
                        new_scheme,
                        default_id: 2,
                    },
                    raw,
                    location,
                )?;
            } else if matches!(id >> 8, 0xc4 | 0xc5 | 0xc7 | 0xc9) {
                return Err(
                    Self::unsupported(location, 4, "spec naming owner is unimplemented").into(),
                );
            }
            id = id.wrapping_add(1);
        }
        Ok(())
    }
}
