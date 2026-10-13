use super::{
    load_currency_native::{Result, setup},
    load_currency_properties_native::{Operation, compare_api},
};
use serde_json::json;

type Case = (&'static str, Vec<Operation>);

fn property(property: u8, first: u16, count: u8, reserve: bool, raw: &[u8]) -> Operation {
    Operation::Property {
        grfid: 7,
        property,
        first,
        count,
        reserve,
        raw: raw.to_vec(),
    }
}

fn sample(property: u8) -> Vec<u8> {
    match property {
        0x0b => 65_536_000_u32.to_le_bytes().to_vec(),
        0x0c => vec![b'|', 0xff],
        0x0d => vec![0xed, 0xa0, 0x80, 0],
        0x0e => vec![0xff, b'Z', 1, 0],
        0x0f => vec![0xff, 0xff],
        _ => Vec::new(),
    }
}

pub(super) fn index_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (name, prop) in [
        ("indices-rate", 0x0b),
        ("indices-options", 0x0c),
        ("indices-prefix", 0x0d),
        ("indices-suffix", 0x0e),
        ("indices-euro", 0x0f),
    ] {
        let mut operations = Vec::new();
        for index in 0..=255_u16 {
            for reserve in [true, false] {
                operations.push(property(prop, index, 1, reserve, &sample(prop)));
            }
        }
        cases.push((name, operations));
    }
    cases
}

fn extended() -> Vec<Operation> {
    let mut extended = Vec::new();
    for prop in 0x0b..=0x0f {
        for (first, count) in [(255, 2), (256, 1), (65535, 255), (31, 0)] {
            for reserve in [true, false] {
                extended.push(property(
                    prop,
                    first,
                    count,
                    reserve,
                    &sample(prop).repeat(usize::from(count)),
                ));
            }
        }
    }
    extended
}

fn options() -> Vec<Operation> {
    let mut options = Vec::new();
    for byte in 0..=255_u8 {
        for position in [0, 1] {
            options.push(property(0x0c, 31, 1, false, &[byte, position]));
        }
    }
    options
}

fn ignored_options() -> Vec<Operation> {
    let mut ignored = Vec::new();
    for high in [2, 4, 8, 16, 32, 64, 128, 254] {
        for position in [0, 1] {
            ignored.push(property(0x0c, 31, 1, false, &[b'.', high | position]));
        }
    }
    ignored
}

fn single_byte_symbols() -> Vec<Operation> {
    let mut symbols = Vec::new();
    for byte in 0..=255_u8 {
        for prop in [0x0d, 0x0e] {
            symbols.push(property(prop, 0, 1, false, &[byte, 0, b'X', 1]));
        }
    }
    symbols
}

fn utf8_symbols() -> Vec<Operation> {
    let mut utf8 = Vec::new();
    for raw in [
        [0xed, 0xa0, 0x80, 0],
        [0xed, 0xbf, 0xbf, 0],
        [0xed, 0x9f, 0xbf, 0],
        [0xf0, 0x90, 0x80, 0x80],
        [0xf4, 0x8f, 0xbf, 0xbf],
        [0xf4, 0x90, 0x80, 0x80],
        [0xc2, 0x80, 0, 0],
        [0xc3, 0x9e, b'A', 0],
        [0xdf, 0xbf, 0, 0],
        [0xe0, 0xa0, 0x80, 0],
        [0xee, 0x80, 0x80, 0],
        [0xee, 0x87, 0xbf, 0],
        [0xee, 0x88, 0x80, 0],
        [0xee, 0x8b, 0xbf, 0],
        [0xee, 0x8c, 0x80, 0],
        [0xc0, 0x80, b'A', 0],
        [0xe0, 0x80, 0x80, b'A'],
        [0xf0, 0x80, 0x80, 0x80],
        [0xff, b'A', 1, 0],
        [0xc2, b'A', 0, 0],
        [b'A', 0, b'B', 1],
        [b'A', b'B', 0, 1],
        [b'A', b'B', b'C', 0],
        [b'A', b'B', b'C', b'D'],
        [0, 0, 0, 0],
        [0x7f, 0xc2, 0x80, 0],
        [b'A', b'B', b'C', 0xe2],
    ] {
        for prop in [0x0d, 0x0e] {
            utf8.push(property(prop, 0, 1, false, &raw));
        }
    }
    utf8
}

fn bounds() -> Vec<Operation> {
    let mut bounds = Vec::new();
    for prop in 0x0b..=0x0f {
        let raw = sample(prop);
        for reserve in [true, false] {
            for first in [0, 46, 255] {
                for length in 0..raw.len() {
                    let tail = raw.get(..length).unwrap_or_default();
                    bounds.push(property(prop, first, 1, reserve, tail));
                    let mut partial = raw.clone();
                    partial.extend(tail);
                    bounds.push(property(prop, first, 2, reserve, &partial));
                }
                let mut trailing = raw.clone();
                trailing.push(42);
                bounds.push(property(prop, first, 1, reserve, &trailing));
            }
        }
    }
    bounds
}

fn reset() -> Vec<Operation> {
    let mut reset = Vec::new();
    for prop in 0x0b..=0x0f {
        reset.push(property(prop, 31, 1, false, &sample(prop)));
    }
    reset.push(property(0x0d, 0, 1, false, &[b'A', 0, 0, 0]));
    reset.push(Operation::ResetCurrencies {
        preserve_custom: true,
    });
    reset.push(Operation::ResetCurrencies {
        preserve_custom: false,
    });
    reset
}

fn cases() -> Vec<Case> {
    let mut cases = index_cases();
    cases.push(("extended-indices", extended()));
    cases.push((
        "rate-boundaries",
        [
            0,
            1,
            999,
            1000,
            1001,
            1999,
            2000,
            65_534_999,
            65_535_000,
            65_535_999,
            65_536_000,
            65_536_999,
            u32::MAX,
        ]
        .into_iter()
        .map(|value| property(0x0b, 0, 1, false, &value.to_le_bytes()))
        .collect(),
    ));
    cases.push((
        "euro-boundaries",
        [0_u16, 1, 2, 1999, 2000, 2002, 65535]
            .into_iter()
            .flat_map(|year| {
                [18, 31].map(|index| property(0x0f, index, 1, false, &year.to_le_bytes()))
            })
            .collect(),
    ));
    cases.extend([
        ("options-bytes", options()),
        ("options-ignored-bits", ignored_options()),
        ("symbols-single-byte", single_byte_symbols()),
        ("symbols-utf8", utf8_symbols()),
        ("reader-bounds", bounds()),
        ("custom-reset-values", reset()),
    ]);
    cases
}

#[test]
#[ignore = "requires complete original byte-mode property API corpus"]
fn original_currency_property_api_matrix() -> Result {
    let (root, directory, oracle, pack) = setup()?;
    let mut summary = Vec::new();
    for (name, operations) in cases() {
        let case = directory.join(name);
        std::fs::create_dir(&case)?;
        let controls = compare_api(&root, &case, &oracle, &pack, &operations)?;
        summary.push(json!({"case":name,"operations":operations.len(),"controls":controls}));
    }
    std::fs::write(
        directory.join("summary.json"),
        serde_json::to_vec(&summary)?,
    )?;
    println!("compared {} original property API groups", summary.len());
    Ok(())
}
