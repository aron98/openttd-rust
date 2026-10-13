use super::{
    load_context_tests::grf_control_cases as programs,
    load_currency_load_native::{Case, OwnerEncoding, run_cases},
    load_currency_native::Result,
};

fn property(first: u16, prop: u8, count: u8, raw: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, 8, 1, count, 255];
    bytes.extend(first.to_le_bytes());
    bytes.push(prop);
    bytes.extend(raw);
    bytes
}

fn values(first: u16, byte: u8) -> Vec<Vec<u8>> {
    vec![
        property(
            first,
            0x0b,
            1,
            &u32::from(byte).saturating_mul(1000).to_le_bytes(),
        ),
        property(first, 0x0c, 1, &[b'|', 0xff]),
        property(first, 0x0d, 1, &[byte, 0, 0, 0]),
        property(first, 0x0e, 1, &[0xed, 0xa0, 0x80, 0]),
        property(first, 0x0f, 1, &2_u16.to_le_bytes()),
    ]
}

fn owner_file(id: u32, byte: u8, version: u8) -> Result<programs::Source> {
    let mut records = vec![
        property(0, 0x0a, 1, &0xd800_u16.to_le_bytes()),
        property(31, 0x0a, 1, &0xd800_u16.to_le_bytes()),
    ];
    records.extend(values(0, byte));
    records.extend(values(31, byte));
    records.push(vec![4, 0, 0xff, 1, 0, 0xd8, byte, 0]);
    programs::source(id, &records, &[], version)
}

fn base_cases() -> Result<Vec<Case>> {
    let first = || owner_file(7, b'A', 1);
    let second = || owner_file(8, b'B', 2);
    let mut packed = vec![0, 8, 5, 1, 0];
    for record in values(0, b'P') {
        packed.extend(record.get(7..).ok_or("property values")?);
    }
    Ok(vec![
        (
            "single-v1",
            vec![programs::source(7, &values(0, b'A'), &[], 1)?],
            None,
        ),
        (
            "single-v2",
            vec![programs::source(7, &values(0, b'B'), &[], 2)?],
            None,
        ),
        (
            "packed-properties",
            vec![programs::source(7, &[packed], &[], 1)?],
            None,
        ),
        ("deferred-and-overwrite", vec![first()?, second()?], None),
        ("overwrite-reverse", vec![second()?, first()?], None),
        (
            "custom-bytes",
            vec![programs::source(7, &values(31, b'C'), &[], 2)?],
            None,
        ),
    ])
}

fn boundary_cases() -> Result<Vec<Case>> {
    let mut init = owner_file(7, b'A', 1)?;
    init.flags.init_only = true;
    Ok(vec![
        (
            "wrapped-and-invalid",
            vec![programs::source(
                7,
                &[
                    property(255, 0x0d, 2, &[0xed, 0xa0, 0x80, 0, b'W', 0, 0, 0]),
                    property(46, 0x0e, 1, &[0xed, 0xbf, 0xbf, 0]),
                ],
                &[],
                1,
            )?],
            None,
        ),
        (
            "maximum-count-wrap",
            vec![programs::source(
                7,
                &[property(
                    65535,
                    0x0b,
                    255,
                    &2000_u32.to_le_bytes().repeat(255),
                )],
                &[],
                2,
            )?],
            None,
        ),
        (
            "zero-items-all-values",
            vec![programs::source(
                7,
                &(0x0b..=0x0f)
                    .map(|p| property(0, p, 0, &[]))
                    .collect::<Vec<_>>(),
                &[],
                1,
            )?],
            None,
        ),
        ("init-only-no-owner-write", vec![init], None),
    ])
}

fn failure_cases() -> Result<Vec<Case>> {
    let mut cases = vec![
        (
            "malformed-reserve-valid-first",
            vec![programs::source(
                7,
                &[
                    property(0, 0x0b, 1, &2000_u32.to_le_bytes()),
                    property(0, 0x0d, 1, b"B"),
                ],
                &[],
                1,
            )?],
            None,
        ),
        (
            "malformed-reserve-invalid-owner",
            vec![programs::source(7, &[property(46, 0x0f, 1, &[1])], &[], 2)?],
            None,
        ),
        (
            "disabled-retained",
            vec![
                programs::source(
                    7,
                    &[
                        property(0, 0x0b, 1, &2000_u32.to_le_bytes()),
                        property(31, 0x0d, 1, &[0xed, 0xa0, 0x80, 0]),
                        vec![0x13, 8, 0, 0, 0],
                    ],
                    &[],
                    1,
                )?,
                programs::source(8, &[], &[], 2)?,
            ],
            None,
        ),
        ("empty-configured-files", vec![], None),
    ];
    for (name, prop, index) in [
        ("activation-truncated-prefix", 0x0d, 0),
        ("activation-truncated-suffix", 0x0e, 255),
    ] {
        cases.push((
            name,
            vec![programs::source(
                7,
                &[
                    property(0, 0x0b, 1, &2000_u32.to_le_bytes()),
                    programs::condition(9, 0x84, 4, 3, 0x201, u32::MAX, 1),
                    property(index, prop, 2, &[b'A', 0, 0, 0, b'B', b'C']),
                ],
                &[],
                1,
            )?],
            None,
        ));
    }
    Ok(cases)
}

fn cases() -> Result<Vec<Case>> {
    let mut cases = base_cases()?;
    cases.extend(boundary_cases()?);
    cases.extend(failure_cases()?);
    let first = || owner_file(7, b'A', 1);
    let second = || owner_file(8, b'B', 2);
    for (name, order) in [
        ("reload-same", vec![0, 1]),
        ("reload-reordered", vec![1, 0]),
        ("reload-remove-first", vec![1]),
        ("reload-remove-second", vec![0]),
        ("reload-empty", vec![]),
    ] {
        cases.push((name, vec![first()?, second()?], Some(order)));
    }
    Ok(cases)
}

#[test]
#[ignore = "requires complete original byte-mode property loader and reload corpus"]
fn original_currency_property_load_matrix() -> Result {
    run_cases(&cases()?, OwnerEncoding::Bytes)
}
