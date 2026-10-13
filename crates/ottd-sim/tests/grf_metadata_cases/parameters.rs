use super::{Cases, Result, action8, binary, branch, info, parameter, text};
/// Generate the complete boundary scenarios.
/// # Errors
/// Returns malformed input, framing overflow or projection errors.
pub fn cases() -> Result<Cases> {
    let mut cases = Vec::new();
    for count in [0, 1, 127, 128, 255] {
        for id in [0_u32, 127, 128, u32::MAX] {
            for before in [false, true] {
                let mut nodes = vec![
                    binary(*b"NPAR", &[count])?,
                    parameter(id, &[binary(*b"DFLT", &[7, 0, 0, 0])?]),
                ];
                if !before {
                    nodes.reverse();
                }
                cases.push((
                    format!("parameter-id-{count}-{id}-{before}"),
                    vec![info(&nodes), action8()],
                ));
            }
        }
    }
    for kind in [0, 1, 2, 255] {
        for (min, max) in [(0_u32, 0_u32), (0, 1), (3, 3), (0, u32::MAX), (9, 2)] {
            for before in [false, true] {
                let mut range = min.to_le_bytes().to_vec();
                range.extend(max.to_le_bytes());
                let mut nodes = vec![binary(*b"TYPE", &[kind])?, binary(*b"LIMI", &range)?];
                if !before {
                    nodes.reverse();
                }
                nodes.push(binary(*b"DFLT", &7_u32.to_le_bytes())?);
                cases.push((
                    format!("parameter-range-{kind}-{min}-{max}-{before}"),
                    vec![info(&[parameter(0, &nodes)]), action8()],
                ));
            }
        }
    }
    for length in [0, 1, 3, 4, 5] {
        cases.push((
            format!("default-length-{length}"),
            vec![
                info(&[
                    parameter(0, &[binary(*b"DFLT", &vec![255; length])?]),
                    parameter(3, &[binary(*b"LIMI", &[2, 0, 0, 0, 4, 0, 0, 0])?]),
                ]),
                action8(),
            ],
        ));
    }
    cases.extend(masks_labels()?);
    cases.extend(complete_labels_and_lengths()?);
    Ok(cases)
}

fn complete_labels_and_lengths() -> Result<Cases> {
    let mut cases = Vec::new();
    for (min, max) in [(0_u32, 0_u32), (1, 1), (1, 3)] {
        for complete in [false, true] {
            let mut range = min.to_le_bytes().to_vec();
            range.extend(max.to_le_bytes());
            let mut labels = Vec::new();
            for value in min..=max {
                if complete || value != max {
                    labels.push(text(value.to_le_bytes(), 1, b"label"));
                }
            }
            cases.push((
                format!("labels-complete-{min}-{max}-{complete}"),
                vec![
                    info(&[parameter(
                        0,
                        &[binary(*b"LIMI", &range)?, branch(*b"VALU", &labels)],
                    )]),
                    action8(),
                ],
            ));
        }
    }
    for tag in [*b"TYPE", *b"LIMI", *b"MASK", *b"DFLT"] {
        for length in 0..=9 {
            cases.push((
                format!("parameter-length-{}-{length}", std::str::from_utf8(&tag)?),
                vec![
                    info(&[parameter(0, &[binary(tag, &vec![1; length])?])]),
                    action8(),
                ],
            ));
        }
    }
    for default in [0_u32, 5, 20] {
        cases.push((
            format!("default-clamp-{default}"),
            vec![
                info(&[parameter(
                    0,
                    &[
                        binary(*b"LIMI", &[3, 0, 0, 0, 9, 0, 0, 0])?,
                        binary(*b"DFLT", &default.to_le_bytes())?,
                    ],
                )]),
                action8(),
            ],
        ));
    }
    Ok(cases)
}
fn masks_labels() -> Result<Cases> {
    let mut cases = Vec::new();
    for slot in [0, 127, 128, 255] {
        for first in [0, 31, 32, 255] {
            for width in [0, 1, 31, 32, 255] {
                cases.push((
                    format!("mask-{slot}-{first}-{width}"),
                    vec![
                        info(&[parameter(
                            0,
                            &[
                                binary(*b"MASK", &[slot, first, width])?,
                                binary(*b"DFLT", &[255; 4])?,
                            ],
                        )]),
                        action8(),
                    ],
                ));
            }
        }
    }
    for length in 0..=4 {
        for first in [0, 31] {
            let mut mask = vec![2, first, 1, 7];
            mask.truncate(length);
            cases.push((
                format!("mask-retain-{length}-{first}"),
                vec![
                    info(&[
                        parameter(
                            0,
                            &[
                                binary(*b"MASK", &[2, 0, 32])?,
                                binary(*b"MASK", &mask)?,
                                binary(*b"DFLT", &[255; 4])?,
                            ],
                        ),
                        parameter(
                            3,
                            &[binary(*b"MASK", &[2, 2, 2])?, binary(*b"DFLT", &[0; 4])?],
                        ),
                    ]),
                    action8(),
                ],
            ));
        }
    }
    for max in [0_u32, 1, 3, u32::MAX] {
        for before in [false, true] {
            let mut range = 1_u32.to_le_bytes().to_vec();
            range.extend(max.to_le_bytes());
            let labels = branch(
                *b"VALU",
                &[
                    text(3_u32.to_le_bytes(), 1, b"three"),
                    text(1_u32.to_le_bytes(), 0, b"one"),
                    text(1_u32.to_le_bytes(), 1, b"ONE"),
                    text(1_u32.to_le_bytes(), 0, b"replacement"),
                    text(0_u32.to_le_bytes(), 127, b"zero"),
                    binary(2_u32.to_le_bytes(), &[])?,
                ],
            );
            let mut nodes = vec![binary(*b"LIMI", &range)?, labels];
            if !before {
                nodes.reverse();
            }
            cases.push((
                format!("labels-{max}-{before}"),
                vec![info(&[parameter(0, &nodes)]), action8()],
            ));
        }
    }
    Ok(cases)
}
