use super::{
    string_ids,
    text::Budget,
    text_choices::Choices,
    text_codes::{
        SCC_BIGFONT, SCC_BLACK, SCC_BLUE, SCC_BROWN, SCC_BUS, SCC_CHECKMARK, SCC_CITY, SCC_CREAM,
        SCC_CROSS, SCC_DKBLUE, SCC_DKGREEN, SCC_DOWN_ARROW, SCC_GENDER_INDEX, SCC_GOLD, SCC_GRAY,
        SCC_GREEN, SCC_LORRY, SCC_LTBLUE, SCC_LTBROWN, SCC_NEWGRF_DISCARD_WORD,
        SCC_NEWGRF_PRINT_BYTE_HEX, SCC_NEWGRF_PRINT_DWORD_DATE_LONG, SCC_NEWGRF_PRINT_DWORD_FORCE,
        SCC_NEWGRF_PRINT_DWORD_HEX, SCC_NEWGRF_PRINT_DWORD_SIGNED, SCC_NEWGRF_PRINT_QWORD_CURRENCY,
        SCC_NEWGRF_PRINT_QWORD_HEX, SCC_NEWGRF_PRINT_WORD_DATE_LONG, SCC_NEWGRF_PRINT_WORD_HEX,
        SCC_NEWGRF_PRINT_WORD_STATION_NAME, SCC_NEWGRF_PRINT_WORD_STRING_ID,
        SCC_NEWGRF_PRINT_WORD_VOLUME_LONG, SCC_NEWGRF_PRINT_WORD_WEIGHT_LONG, SCC_NEWGRF_PUSH_WORD,
        SCC_NEWGRF_ROTATE_TOP_4_WORDS, SCC_NEWGRF_STRINL, SCC_ORANGE, SCC_PLANE, SCC_POP_COLOUR,
        SCC_PURPLE, SCC_PUSH_COLOUR, SCC_RED, SCC_RIGHT_ARROW, SCC_SET_CASE, SCC_SHIP, SCC_SILVER,
        SCC_SMALL_DOWN_ARROW, SCC_SMALL_UP_ARROW, SCC_SUPERSCRIPT_M1, SCC_TINYFONT, SCC_TOWN,
        SCC_TRAIN, SCC_UP_ARROW, SCC_WHITE, SCC_YELLOW,
    },
    text_mapped::{ChoiceKind, TextContext},
    text_reader::{Consumer, encode},
    types::ScanError,
};

#[derive(Clone, Copy)]
pub(super) struct Input<'a> {
    pub raw: &'a [u8],
    pub newlines: bool,
    pub offset: usize,
    pub context: TextContext<'a>,
}
impl<'a> Input<'a> {
    fn reader(self) -> (bool, Consumer<'a>) {
        let unicode = self.raw.starts_with(&[0xc3, 0x9e]);
        let bytes = if unicode {
            self.raw.get(2..).unwrap_or_default()
        } else {
            self.raw
        };
        (unicode, Consumer { bytes })
    }
}

fn emit(
    output: &mut Choices,
    value: u32,
    budget: &mut Budget,
    offset: usize,
) -> Result<(), ScanError> {
    let (bytes, length) = encode(value);
    output.append(bytes.get(..length).unwrap_or_default(), budget, offset)
}
fn printable(value: u32) -> u32 {
    if value < 32 || (0xe000..0xe200).contains(&value) {
        63
    } else {
        value
    }
}

fn extended(
    reader: &mut Consumer<'_>,
    output: &mut Choices,
    budget: &mut Budget,
    input: Input<'_>,
) -> Result<bool, ScanError> {
    let offset = input.offset;
    let code = reader.byte();
    let value = match code {
        0 => return Ok(false),
        1 => SCC_NEWGRF_PRINT_QWORD_CURRENCY,
        3 => {
            let value = u32::from(reader.word());
            emit(output, SCC_NEWGRF_PUSH_WORD, budget, offset)?;
            value
        }
        6 => SCC_NEWGRF_PRINT_BYTE_HEX,
        7 => SCC_NEWGRF_PRINT_WORD_HEX,
        8 => SCC_NEWGRF_PRINT_DWORD_HEX,
        0x0b => SCC_NEWGRF_PRINT_QWORD_HEX,
        0x0c => SCC_NEWGRF_PRINT_WORD_STATION_NAME,
        0x0d => SCC_NEWGRF_PRINT_WORD_WEIGHT_LONG,
        0x0e | 0x0f => {
            let index = reader.byte();
            if let Some(mapped) = input
                .context
                .map
                .and_then(|map| map.forward(index, code == 0x0e))
            {
                emit(
                    output,
                    if code == 0x0e {
                        SCC_GENDER_INDEX
                    } else {
                        SCC_SET_CASE
                    },
                    budget,
                    offset,
                )?;
                emit(
                    output,
                    u32::from(mapped).saturating_add(u32::from(code == 0x0f)),
                    budget,
                    offset,
                )?;
            }
            return Ok(true);
        }
        0x10 | 0x11 => {
            let index = if code == 0x10 { reader.byte() } else { 0 };
            output.next(index);
            return Ok(true);
        }
        0x12 => {
            output.finish(input.context, budget, offset)?;
            return Ok(true);
        }
        0x13..=0x15 => {
            let list_offset = if code == 0x14 { 0 } else { reader.byte() };
            let kind = match code {
                0x13 => ChoiceKind::Gender,
                0x14 => ChoiceKind::Case,
                _ => ChoiceKind::Plural,
            };
            output.start(kind, list_offset);
            return Ok(true);
        }
        0x16..=0x1e => {
            SCC_NEWGRF_PRINT_DWORD_DATE_LONG.saturating_add(u32::from(code.saturating_sub(0x16)))
        }
        0x1f => SCC_PUSH_COLOUR,
        0x20 => SCC_POP_COLOUR,
        0x21 => SCC_NEWGRF_PRINT_DWORD_FORCE,
        _ => return Ok(true),
    };
    emit(output, value, budget, offset)?;
    Ok(true)
}

pub(super) fn translate(
    raw: &[u8],
    newlines: bool,
    budget: &mut Budget,
    offset: usize,
) -> Result<Vec<u8>, ScanError> {
    translate_in(
        Input {
            raw,
            newlines,
            offset,
            context: TextContext::default(),
        },
        budget,
        |id| Some(string_ids::map(id)),
    )
}

pub(super) fn translate_in(
    input: Input<'_>,
    budget: &mut Budget,
    mut resolve: impl FnMut(u16) -> Option<u32>,
) -> Result<Vec<u8>, ScanError> {
    let Input {
        newlines, offset, ..
    } = input;
    let (unicode, mut reader) = input.reader();
    let mut output = Choices::default();
    while !reader.bytes.is_empty() {
        let value = if let Some(value) = if unicode { reader.unicode() } else { None } {
            if (0xe000..=0xe0ff).contains(&value) {
                value.saturating_sub(0xe000)
            } else if value >= 32 {
                emit(&mut output, printable(value), budget, offset)?;
                continue;
            } else {
                value
            }
        } else {
            u32::from(reader.byte())
        };
        let value = match value {
            0 => break,
            1 => {
                reader.skip(1);
                32
            }
            10 => continue,
            13 => {
                if !newlines {
                    continue;
                }
                10
            }
            14 => SCC_TINYFONT,
            15 => SCC_BIGFONT,
            31 => {
                reader.skip(2);
                32
            }
            0x7b..=0x7f => SCC_NEWGRF_PRINT_DWORD_SIGNED.saturating_add(value.saturating_sub(0x7b)),
            0x80 => SCC_NEWGRF_PRINT_WORD_STRING_ID,
            0x81 => {
                let id = reader.word();
                emit(&mut output, SCC_NEWGRF_STRINL, budget, offset)?;
                resolve(id).ok_or(ScanError::UnsupportedInline { id })?
            }
            0x82..=0x84 => {
                SCC_NEWGRF_PRINT_WORD_DATE_LONG.saturating_add(value.saturating_sub(0x82))
            }
            0x85 => SCC_NEWGRF_DISCARD_WORD,
            0x86 => SCC_NEWGRF_ROTATE_TOP_4_WORDS,
            0x87 => SCC_NEWGRF_PRINT_WORD_VOLUME_LONG,
            0x88 => SCC_BLUE,
            0x89 => SCC_SILVER,
            0x8a => SCC_GOLD,
            0x8b => SCC_RED,
            0x8c => SCC_PURPLE,
            0x8d => SCC_LTBROWN,
            0x8e => SCC_ORANGE,
            0x8f => SCC_GREEN,
            0x90 => SCC_YELLOW,
            0x91 => SCC_DKGREEN,
            0x92 => SCC_CREAM,
            0x93 => SCC_BROWN,
            0x94 => SCC_WHITE,
            0x95 => SCC_LTBLUE,
            0x96 => SCC_GRAY,
            0x97 => SCC_DKBLUE,
            0x98 => SCC_BLACK,
            0x9a => {
                if !extended(&mut reader, &mut output, budget, input)? {
                    break;
                }
                continue;
            }
            0x9b => SCC_TOWN,
            0x9c => SCC_CITY,
            0x9e => 0x20ac,
            0x9f => 0x178,
            0xa0 => SCC_UP_ARROW,
            0xaa => SCC_DOWN_ARROW,
            0xac => SCC_CHECKMARK,
            0xad => SCC_CROSS,
            0xaf => SCC_RIGHT_ARROW,
            0xb4 => SCC_TRAIN,
            0xb5 => SCC_LORRY,
            0xb6 => SCC_BUS,
            0xb7 => SCC_PLANE,
            0xb8 => SCC_SHIP,
            0xb9 => SCC_SUPERSCRIPT_M1,
            0xbc => SCC_SMALL_UP_ARROW,
            0xbd => SCC_SMALL_DOWN_ARROW,
            _ => printable(value),
        };
        emit(&mut output, value, budget, offset)?;
    }
    Ok(output.output)
}
