use super::{Case, Result, condition, encode, info, set, single, source};

pub(super) fn cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for width in [3, 255] {
        cases.push(single(
            &format!("condition-width-{width}"),
            &[condition(9, 0, width, 2, 0, 0, 1), set(1, 0, 255, 255, 7)],
            &[5],
            2,
        )?);
    }
    for (label, count) in [(0, 0), (255, 255)] {
        cases.push(single(
            &format!("label-{label}"),
            &[
                condition(9, 0, 1, 2, 1, 255, count),
                vec![0],
                vec![0x10, label],
            ],
            &[1],
            1,
        )?);
    }
    for bit in [0, 31] {
        for kind in [0, 1] {
            cases.push(single(
                &format!("bit-{bit}-{kind}"),
                &[
                    condition(9, 0, 4, kind, bit, u32::MAX, 255),
                    set(1, 0, 255, 255, 7),
                ],
                &[0x8000_0001],
                1,
            )?);
        }
    }
    for size in [0_usize, 1, 2, 3, 4] {
        let mut record = vec![0x0d, 0, 0, 255, 255];
        record.extend([1, 2, 3, 4].into_iter().take(size));
        cases.push(single(&format!("optional-data-{size}"), &[record], &[], 1)?);
    }
    for shift in [0, 1, 31, 32, 33, 0xffff_ffff, 0xffff_ffe1] {
        for op in [5, 6] {
            cases.push(single(
                &format!("shift-{op}-{shift:08x}"),
                &[set(2, op, 0, 1, 0)],
                &[0x8000_0001, shift],
                2,
            )?);
        }
    }
    for size in [1_usize, 3, 4, 5, 8, 127] {
        for add in [false, true] {
            let mut target = vec![0x0c];
            target.extend([255; 140]);
            cases.push(single(
                &format!("words-{size}-{add}"),
                &[
                    vec![
                        6,
                        0,
                        u8::try_from(size)? | if add { 128 } else { 0 },
                        1,
                        255,
                    ],
                    target,
                ],
                &[0xffff_ffff; 32],
                2,
            )?);
        }
    }
    for offset in [254_u16, 255, 256, 65535] {
        let mut modifier = vec![6, 0, 4, 255];
        modifier.extend(offset.to_le_bytes());
        modifier.push(255);
        let mut target = vec![0x0c];
        target.extend([0; 260]);
        cases.push(single(
            &format!("extended-{offset}"),
            &[modifier, target],
            &[0x1234_5678],
            2,
        )?);
    }
    cases.extend(physical_limits()?);
    cases.extend(registry()?);
    cases.extend(real_sprites()?);
    Ok(cases)
}
fn physical_limits() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for length in [1024 * 1024, 1024 * 1024 + 1] {
        let mut record = vec![0; length];
        *record.first_mut().ok_or("large action")? = 0x0c;
        cases.push(single(
            &format!("pseudo-length-{length}"),
            &[record],
            &[],
            2,
        )?);
    }
    let mut huge = source(
        0x4141_4141,
        &[vec![1, 0, 0, 0, 255, 255, 255, 255, 255, 255]],
        &[],
        1,
    )?;
    huge.flags.init_only = true;
    cases.push(Case {
        name: "signed-skip-counter".into(),
        sources: vec![huge],
        networking: false,
    });
    Ok(cases)
}
fn registry() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for id in [0x4343_4343, 0x4242_4242] {
        let a = source(0x4141_4141, &[set(0, 0, 0, 254, id)], &[], 1)?;
        let mut b = source(0x4242_4242, &[info(0x4242_4242)], &[99], 1)?;
        if id == 0x4343_4343 {
            b.bytes = None;
        }
        cases.push(Case {
            name: format!("external-absent-disabled-{id}"),
            sources: vec![b, a],
            networking: false,
        });
    }
    let a = source(0x4141_4141, &[set(0, 1, 0, 254, 0x4242_4242)], &[1], 1)?;
    let b = source(0x4242_4242, &[set(0, 1, 0, 254, 0x4141_4141)], &[2], 1)?;
    let c = source(
        0x4343_4343,
        &[
            condition(9, 0x88, 8, 9, 0x4242_4240, 0xffff_fff0, 1),
            set(0, 0, 255, 255, 99),
        ],
        &[],
        1,
    )?;
    cases.push(Case {
        name: "three-files-mutual-masked".into(),
        sources: vec![a, b, c],
        networking: false,
    });
    let a = source(0x4141_4141, &[set(0, 1, 0, 255, 1)], &[1], 1)?;
    let b = source(0x4141_4141, &[set(0, 1, 0, 255, 1)], &[99], 1)?;
    cases.push(Case {
        name: "filename-reuse".into(),
        sources: vec![a, b],
        networking: false,
    });
    for count in [255_u32, 256] {
        let mut sources = Vec::new();
        for index in 0..count {
            sources.push(source(
                0x4141_0000_u32.saturating_add(index.saturating_mul(256)),
                &[],
                &[],
                1,
            )?);
        }
        cases.push(Case {
            name: format!("non-static-count-{count}"),
            sources,
            networking: false,
        });
    }
    cases.extend(duplicate_lookup()?);
    Ok(cases)
}
fn duplicate_lookup() -> Result<Vec<Case>> {
    let first = source(0x4141_4141, &[], &[11], 1)?;
    let mut later = source(0x4141_4141, &[], &[99], 1)?;
    later.metadata_version = 99;
    let reader = source(
        0x4242_4242,
        &[
            condition(9, 0x88, 8, 8, 0x4141_4140, 0xffff_fff0, 1),
            set(0, 0, 255, 255, 9),
            set(1, 0, 254, 254, first.id),
        ],
        &[0],
        1,
    )?;
    let mut cases = vec![Case {
        name: "duplicate-first-config-version-status".into(),
        sources: vec![first, reader, later],
        networking: false,
    }];
    for system in [false, true] {
        let mut sources = Vec::new();
        for index in 0..256_u32 {
            sources.push(source(
                0x4141_0000_u32.saturating_add(index.saturating_mul(256)),
                &[],
                &[],
                1,
            )?);
        }
        let last = sources.last_mut().ok_or("last count config")?;
        last.flags.system = system;
        last.flags.is_static = !system;
        cases.push(Case {
            name: format!("non-static-exemption-system-{system}"),
            sources,
            networking: false,
        });
    }
    Ok(cases)
}
fn real_sprites() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for (name, flags, physical) in [
        ("raw", 2, vec![1, 2, 3, 4]),
        ("compressed", 0, vec![1, 42, 0xe8, 1]),
    ] {
        let mut file = source(0x4141_4141, &[], &[], 1)?;
        let mut bytes = encode(
            &[
                info(file.id),
                condition(9, 0x84, 4, 2, 0x201, u32::MAX, 2),
                vec![1, 0, 1, 1],
            ],
            1,
        )?;
        bytes.truncate(bytes.len().saturating_sub(2));
        bytes.extend(12_u16.to_le_bytes());
        bytes.push(flags);
        bytes.extend([0; 7]);
        bytes.extend(physical);
        bytes.extend([2, 0, 255, 0x10, 42, 0, 0]);
        file.bytes = Some(bytes);
        cases.push(Case {
            name: format!("skip-real-{name}-label"),
            sources: vec![file],
            networking: false,
        });
    }
    let mut file = source(0x4141_4141, &[], &[], 2)?;
    let mut bytes = encode(
        &[
            info(file.id),
            condition(9, 0x84, 4, 2, 0x201, u32::MAX, 2),
            vec![5, 0, 1],
        ],
        2,
    )?;
    bytes.truncate(bytes.len().saturating_sub(8));
    bytes.extend(4_u32.to_le_bytes());
    bytes.push(253);
    bytes.extend(7_u32.to_le_bytes());
    bytes.extend([0; 4]);
    let offset = u32::try_from(bytes.len().checked_sub(14).ok_or("offset")?)?;
    bytes
        .get_mut(10..14)
        .ok_or("offset field")?
        .copy_from_slice(&offset.to_le_bytes());
    bytes.extend([0; 4]);
    file.bytes = Some(bytes);
    cases.push(Case {
        name: "skip-v2-reference".into(),
        sources: vec![file],
        networking: false,
    });
    Ok(cases)
}
