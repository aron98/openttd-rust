use super::{
    language_pack::PLURAL_FORMS,
    load_language::LanguageMap,
    text::Budget,
    text_codes::{SCC_GENDER_LIST, SCC_PLURAL_LIST, SCC_SWITCH_CASE},
    text_reader::encode,
    types::ScanError,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default)]
pub(super) struct TextContext<'a> {
    pub map: Option<&'a LanguageMap>,
    pub genders: u8,
    pub cases: u8,
}

#[derive(Clone, Copy)]
pub(super) enum ChoiceKind {
    Gender,
    Case,
    Plural,
}

pub(super) struct ChoiceList {
    pub kind: ChoiceKind,
    pub offset: u8,
    pub strings: BTreeMap<u8, Vec<u8>>,
}

struct Output<'a> {
    bytes: Vec<u8>,
    budget: &'a mut Budget,
    offset: usize,
}
impl Output<'_> {
    fn append(&mut self, bytes: &[u8]) -> Result<(), ScanError> {
        self.budget.emit(bytes.len(), self.offset)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn code(&mut self, code: u32) -> Result<(), ScanError> {
        let (bytes, count) = encode(code);
        self.append(bytes.get(..count).unwrap_or_default())
    }
    fn case(&mut self, bytes: &[u8]) -> Result<(), ScanError> {
        let length = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
        self.append(&length.to_le_bytes())?;
        self.append(bytes.get(..usize::from(length)).unwrap_or_default())
    }
}

impl ChoiceList {
    pub(super) fn mapped(
        &self,
        context: TextContext<'_>,
        budget: &mut Budget,
        offset: usize,
    ) -> Result<Vec<u8>, ScanError> {
        let Some(map) = context.map else {
            return Ok(Vec::new());
        };
        let default = self.strings.get(&0).map_or(&[][..], Vec::as_slice);
        let mut output = Output {
            bytes: Vec::new(),
            budget,
            offset,
        };
        match self.kind {
            ChoiceKind::Case => {
                output.code(SCC_SWITCH_CASE)?;
                let count = (0..context.cases)
                    .filter(|&index| {
                        map.reverse(index, false)
                            .is_some_and(|key| self.strings.contains_key(&key))
                    })
                    .count();
                output.append(&[u8::try_from(count).unwrap_or(u8::MAX)])?;
                for index in 0..context.cases {
                    if let Some(bytes) = map
                        .reverse(index, false)
                        .and_then(|key| self.strings.get(&key))
                    {
                        output.append(&[index.saturating_add(1)])?;
                        output.case(bytes)?;
                    }
                }
                output.case(default)?;
            }
            ChoiceKind::Gender | ChoiceKind::Plural => {
                let gender = matches!(self.kind, ChoiceKind::Gender);
                output.code(if gender {
                    SCC_GENDER_LIST
                } else {
                    SCC_PLURAL_LIST
                })?;
                if !gender {
                    output.append(&[map.plural])?;
                }
                let count = if gender {
                    context.genders
                } else {
                    PLURAL_FORMS
                };
                output.append(&[self.offset.wrapping_sub(0x80), count])?;
                for index in 0..count {
                    let key = if gender {
                        map.reverse(index, true)
                    } else {
                        Some(index.saturating_add(1))
                    };
                    let bytes = key
                        .and_then(|k| self.strings.get(&k))
                        .map_or(default, Vec::as_slice);
                    output.append(&[u8::try_from(bytes.len()).unwrap_or(u8::MAX)])?;
                }
                for index in 0..count {
                    let key = if gender {
                        map.reverse(index, true)
                    } else {
                        Some(index.saturating_add(1))
                    };
                    let bytes = key
                        .and_then(|k| self.strings.get(&k))
                        .map_or(default, Vec::as_slice);
                    output.append(bytes.get(..bytes.len().min(255)).unwrap_or_default())?;
                }
            }
        }
        Ok(output.bytes)
    }
}
