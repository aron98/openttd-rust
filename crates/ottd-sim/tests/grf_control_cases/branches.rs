use super::{Case, Result, condition, encode, info, set, single, source};
pub(super) fn cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for action in [7, 9] {
        for width in [1, 2, 4, 8, 0] {
            for kind in 0..=5 {
                cases.push(single(
                    &format!("condition-{action}-{width}-{kind}"),
                    &[
                        condition(action, 0, width, kind, 1, 3, 1),
                        set(1, 1, 1, 255, 1),
                    ],
                    &[3, 0],
                    1,
                )?);
            }
        }
    }
    for (name, records, params) in [
        ("false-missing-count", vec![vec![9, 0, 1, 2, 8]], vec![7]),
        ("true-missing-count", vec![vec![9, 0, 1, 2, 7]], vec![7]),
        ("undefined-condition", vec![vec![9, 0, 1, 2, 7]], vec![]),
        ("unknown-condition", vec![vec![9, 0, 1, 0x13, 7]], vec![7]),
        (
            "duplicate-forward-label",
            vec![
                condition(9, 0, 1, 2, 1, 255, 42),
                set(1, 0, 255, 255, 10),
                vec![0x10, 42],
                set(1, 0, 255, 255, 20),
                vec![0x10, 42],
            ],
            vec![1],
        ),
        (
            "backward-loop",
            vec![
                vec![0x10, 42],
                set(0, 1, 0, 255, 1),
                condition(9, 0, 1, 4, 3, 255, 42),
            ],
            vec![0],
        ),
        (
            "stop-after-info",
            vec![condition(9, 0, 1, 2, 1, 255, 0), vec![0]],
            vec![1],
        ),
        ("ignored-malformed-seven", vec![vec![7]], vec![]),
    ] {
        cases.push(single(name, &records, &params, 2)?);
    }
    let mut before = source(0x4141_4141, &[], &[1], 1)?;
    before.bytes = Some(encode(
        &[condition(9, 0, 1, 2, 1, 255, 0), info(before.id)],
        1,
    )?);
    cases.push(Case {
        name: "stop-before-info".into(),
        sources: vec![before],
        networking: false,
    });
    for kind in 6..=10 {
        for order in [false, true] {
            let mut sources = vec![
                source(
                    0x4141_4141,
                    &[
                        condition(9, 0x88, 8, kind, 0x4242_4242, u32::MAX, 1),
                        set(0, 1, 0, 255, 1),
                    ],
                    &[0],
                    1,
                )?,
                source(0x4242_4242, &[], &[], 1)?,
            ];
            if order {
                sources.reverse();
            }
            cases.push(Case {
                name: format!("registry-{kind}-{order}"),
                sources,
                networking: false,
            });
        }
    }
    Ok(cases)
}
