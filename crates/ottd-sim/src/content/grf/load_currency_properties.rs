// SPDX-License-Identifier: GPL-2.0-only
// OpenTTD 14ec60f: newgrf_act0_globalvar.cpp 0B-0F; string.cpp StrMakeValid.
use super::{
    load::{ActionResult, Session},
    load_currency::{CurrencyIndex, CurrencyOwners, CurrencyState},
    load_types::{LoadLocation, LoadStage},
    records::Reader,
    text_reader::{Consumer, encode},
    types::GrfParseError,
};

#[derive(Clone, Copy)]
pub(super) enum Property {
    Rate,
    Options,
    Prefix,
    Suffix,
    Euro,
}

pub(super) enum Value {
    Rate(u16),
    Options(u16),
    Prefix([u8; 4]),
    Suffix([u8; 4]),
    Euro(u16),
}

impl Property {
    pub(super) const fn from_id(id: u8) -> Option<Self> {
        match id {
            0x0b => Some(Self::Rate),
            0x0c => Some(Self::Options),
            0x0d => Some(Self::Prefix),
            0x0e => Some(Self::Suffix),
            0x0f => Some(Self::Euro),
            _ => None,
        }
    }

    pub(super) fn read(
        self,
        reader: &mut Reader<'_>,
        activation: bool,
    ) -> Result<Option<Value>, GrfParseError> {
        if !activation {
            match self {
                Self::Rate | Self::Prefix | Self::Suffix => {
                    reader.dword()?;
                }
                Self::Options | Self::Euro => {
                    reader.word()?;
                }
            }
            return Ok(None);
        }
        Ok(Some(match self {
            Self::Rate => {
                let [low, high, _, _] = (reader.dword()? / 1000).to_le_bytes();
                Value::Rate(u16::from_le_bytes([low, high]))
            }
            Self::Options => Value::Options(reader.word()?),
            Self::Prefix => Value::Prefix([
                reader.byte()?,
                reader.byte()?,
                reader.byte()?,
                reader.byte()?,
            ]),
            Self::Suffix => Value::Suffix([
                reader.byte()?,
                reader.byte()?,
                reader.byte()?,
                reader.byte()?,
            ]),
            Self::Euro => Value::Euro(reader.word()?),
        }))
    }
}

fn symbol(raw: [u8; 4]) -> Vec<u8> {
    let mut reader = Consumer { bytes: &raw };
    let mut output = Vec::with_capacity(4);
    while !reader.bytes.is_empty() {
        let Some(value) = reader.unicode() else {
            reader.skip(1);
            continue;
        };
        if value == 0 {
            break;
        }
        let value = if value < 0x20 || (0xe000..=0xe2ff).contains(&value) {
            u32::from(b'?')
        } else {
            value
        };
        let (bytes, length) = encode(value);
        output.extend(bytes.into_iter().take(length));
    }
    output
}

impl Value {
    pub(super) fn apply(self, currency: &mut CurrencyState, index: u32) {
        let owner = currency
            .owners
            .entries
            .get_mut(CurrencyIndex::from_grf(index).offset());
        match self {
            Self::Rate(rate) => {
                if let Some(owner) = owner {
                    owner.rate = rate;
                }
            }
            Self::Options(options) => {
                if let Some(owner) = owner {
                    owner.separator = match options.to_le_bytes()[0] {
                        0 | 128..=255 => String::new(),
                        1..=31 => "?".to_owned(),
                        byte => char::from(byte).to_string(),
                    };
                    owner.symbol_pos = options.to_le_bytes()[1] & 1;
                }
            }
            Self::Prefix(raw) => {
                let value = symbol(raw);
                if let Some(owner) = owner {
                    owner.prefix = value;
                }
            }
            Self::Suffix(raw) => {
                let value = symbol(raw);
                if let Some(owner) = owner {
                    owner.suffix = value;
                }
            }
            Self::Euro(year) => {
                if let Some(owner) = owner {
                    owner.to_euro = i32::from(year);
                }
            }
        }
    }
}

impl Session<'_, '_> {
    pub(super) fn currency_properties(
        &mut self,
        reader: &mut Reader<'_>,
        property: Property,
        first: u32,
        items: u32,
        location: LoadLocation,
    ) -> ActionResult {
        for index in first..first.saturating_add(items) {
            let start = reader.pos;
            if let Some(value) = property.read(reader, location.stage == LoadStage::Activation)? {
                self.budget
                    .payload(reader.pos.saturating_sub(start), location)?;
                if self.currency.is_none() {
                    self.budget
                        .payload(CurrencyOwners::initial_bytes(), location)?;
                }
                value.apply(
                    self.currency.get_or_insert_with(CurrencyState::default),
                    index,
                );
            }
        }
        Ok(())
    }
}
