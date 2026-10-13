// SPDX-License-Identifier: GPL-2.0-only
// OpenTTD 14ec60f248547d4d062a1160f0fc26d742319888: language.h, strings.cpp, strgen.cpp.
use super::{records::Reader, text_reader::Consumer};

pub(super) const PACK_VERSION: u32 = 0x2ad1_09ab;
pub(super) const PLURAL_RULES: u8 = 15;
pub(super) const PLURAL_FORMS: u8 = 5;
const HEADER_BYTES: usize = 572;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(super) enum PackError {
    #[error("invalid original language header")]
    Header,
    #[error("invalid original language body")]
    Body,
    #[error("host admission refuses unterminated language name slot")]
    NameSlot,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct Pack {
    pub language: u8,
    pub gender_count: u8,
    pub case_count: u8,
    pub plural: u8,
    pub genders: [[u8; 16]; 8],
    pub cases: [[u8; 16]; 16],
    pub tables: [u16; 32],
}

fn valid_header_string(bytes: &[u8]) -> bool {
    let mut input = Consumer { bytes };
    while !input.bytes.is_empty() {
        let Some(value) = input.unicode() else {
            return false;
        };
        if value == 0 {
            return true;
        }
        if value < 32 || (0xe000..=0xe2ff).contains(&value) {
            return false;
        }
    }
    false
}

impl Pack {
    pub(super) fn header(bytes: &[u8]) -> Result<Self, PackError> {
        Self::read_header(bytes).map_err(|_| PackError::Header)
    }

    fn read_header(bytes: &[u8]) -> Result<Self, super::GrfParseError> {
        let invalid = super::GrfParseError::InitialRecord;
        let mut reader = Reader { bytes, pos: 0 };
        if reader.dword()? != 0x474e_414c || reader.dword()? != PACK_VERSION {
            return Err(invalid);
        }
        for count in [32, 32, 16] {
            if !valid_header_string(reader.take(count)?) {
                return Err(invalid);
            }
        }
        let mut tables = [0; 32];
        for value in &mut tables {
            *value = reader.word()?;
        }
        for _ in 0..3 {
            if !valid_header_string(reader.take(8)?) {
                return Err(invalid);
            }
        }
        reader.word()?;
        let plural = reader.byte()?;
        let direction = reader.byte()?;
        reader.word()?;
        let language = reader.byte()?;
        let gender_count = reader.byte()?;
        let case_count = reader.byte()?;
        reader.take(3)?;
        let mut genders = [[0; 16]; 8];
        for value in &mut genders {
            value.copy_from_slice(reader.take(16)?);
        }
        let mut cases = [[0; 16]; 16];
        for value in &mut cases {
            value.copy_from_slice(reader.take(16)?);
        }
        if plural >= PLURAL_RULES
            || direction > 1
            || language >= 0x7f
            || gender_count >= 8
            || case_count >= 16
        {
            return Err(invalid);
        }
        Ok(Self {
            language,
            gender_count,
            case_count,
            plural,
            genders,
            cases,
            tables,
        })
    }

    pub(super) fn admit_names(&self) -> Result<(), PackError> {
        if self
            .genders
            .iter()
            .chain(self.cases.iter())
            .any(|name| !name.contains(&0))
        {
            return Err(PackError::NameSlot);
        }
        Ok(())
    }

    pub(super) fn body(&self, bytes: &[u8]) -> Result<(), PackError> {
        self.visit_body(bytes, |_| {})
    }

    fn visit_body(
        &self,
        bytes: &[u8],
        mut string: impl FnMut(std::ops::Range<usize>),
    ) -> Result<(), PackError> {
        if bytes.len() > 1 << 20 || self.tables.iter().any(|&count| count > 2048) {
            return Err(PackError::Body);
        }
        let mut position = HEADER_BYTES;
        for _ in 0..self.tables.iter().map(|&n| usize::from(n)).sum() {
            let first = bytes.get(position).copied().unwrap_or(0);
            position = position.checked_add(1).ok_or(PackError::Body)?;
            let mut length = usize::from(first);
            if position.saturating_add(length) > bytes.len() {
                return Err(PackError::Body);
            }
            if first >= 0xc0 {
                let second = bytes.get(position).copied().unwrap_or(0);
                position = position.checked_add(1).ok_or(PackError::Body)?;
                length = (usize::from(first & 63) << 8) | usize::from(second);
                if position.saturating_add(length) > bytes.len() {
                    return Err(PackError::Body);
                }
            }
            let end = position.checked_add(length).ok_or(PackError::Body)?;
            string(position..end);
            position = end;
        }
        Ok(())
    }

    pub(super) fn name_index(&self, name: &[u8], gender: bool) -> Option<u8> {
        let slots: &[[u8; 16]] = if gender { &self.genders } else { &self.cases };
        slots
            .iter()
            .position(|slot| {
                slot.get(..slot.iter().position(|&v| v == 0).unwrap_or(slot.len())) == Some(name)
            })
            .and_then(|index| u8::try_from(index).ok())
    }
}

#[derive(Debug)]
pub(super) struct BuiltinPack<'a> {
    bytes: &'a [u8],
    starts: [usize; 32],
    strings: Vec<std::ops::Range<usize>>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("builtin lookup cannot resolve string domain {0:#x}")]
pub(super) struct LookupDomain(pub u32);

impl<'a> BuiltinPack<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Result<Self, PackError> {
        let pack = Pack::header(bytes)?;
        pack.body(bytes)?;
        let mut strings = Vec::new();
        pack.visit_body(bytes, |range| strings.push(range))?;
        let mut starts = [0; 32];
        let mut total = 0_usize;
        for (start, &count) in starts.iter_mut().zip(&pack.tables) {
            *start = total;
            total = total.saturating_add(usize::from(count));
        }
        Ok(Self {
            bytes,
            starts,
            strings,
        })
    }

    pub(super) fn lookup(&self, id: u32) -> Result<&'a [u8], LookupDomain> {
        let tab = id >> 11;
        if tab == 26 || tab >= 32 {
            return Err(LookupDomain(id));
        }
        let table = usize::try_from(tab).map_err(|_| LookupDomain(id))?;
        let index = usize::try_from(id & 2047).map_err(|_| LookupDomain(id))?;
        let start = self.starts.get(table).ok_or(LookupDomain(id))?;
        let offset = start.saturating_add(index);
        self.strings
            .get(offset)
            .map_or(Ok(b"(undefined string)"), |range| {
                self.bytes.get(range.clone()).ok_or(LookupDomain(id))
            })
    }
}
