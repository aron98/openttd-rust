use super::{Case, Result, condition, info, set, single, source};
pub(super) fn cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for version in [1, 2] {
        for (name, modifier, params) in [
            ("replace", vec![6, 0, 4, 5, 255], vec![0x1234_5678]),
            ("add", vec![6, 0, 0x84, 5, 255], vec![1]),
            ("truncate", vec![6, 0, 4, 8, 255], vec![0x1234_5678]),
            ("outside", vec![6, 0, 4, 99, 255], vec![1]),
            ("extended-offset", vec![6, 0, 4, 255, 5, 0, 255], vec![9]),
            ("zero-size", vec![6, 0, 0, 5, 0, 4, 5, 255], vec![9]),
            ("undefined-break", vec![6, 1, 4, 5, 0, 4, 5, 255], vec![9]),
            (
                "overlap",
                vec![6, 0, 2, 5, 1, 2, 6, 255],
                vec![0xffff, 0x1234],
            ),
            ("partial-bounds", vec![6, 0, 1, 5, 0], vec![9]),
            ("phase-variable", vec![6, 0x84, 4, 5, 255], vec![]),
        ] {
            cases.push(single(
                &format!("substitute-{version}-{name}"),
                &[modifier, set(2, 0, 255, 255, 0)],
                &params,
                version,
            )?);
        }
    }
    cases.push(single(
        "modified-action",
        &[vec![6, 0, 1, 0, 255], vec![0, 1, 0, 255, 255, 7, 0, 0, 0]],
        &[13],
        1,
    )?);
    cases.push(single("duplicate-info", &[info(0x4141_4141)], &[], 1)?);
    cases.push(single("malformed-label", &[vec![0x10]], &[], 1)?);
    cases.push(single(
        "unknown-pseudo",
        &[vec![0x15], vec![0xfe], vec![0xff], vec![0x14], vec![0x0c]],
        &[],
        2,
    )?);
    let mut only = source(0x4141_4141, &[set(0, 1, 0, 255, 1)], &[0], 1)?;
    only.flags.init_only = true;
    cases.push(Case {
        name: "init-only".into(),
        sources: vec![only],
        networking: false,
    });
    let mut missing = source(0x4141_4141, &[], &[9], 1)?;
    missing.bytes = None;
    cases.push(Case {
        name: "missing".into(),
        sources: vec![missing],
        networking: false,
    });
    cases.extend(external()?);
    Ok(cases)
}
fn external() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for networking in [false, true] {
        for version_query in [false, true] {
            for reverse in [false, true] {
                let mut target = source(0x4242_4242, &[set(0, 1, 0, 255, 1)], &[7], 2)?;
                target.flags.is_static = true;
                let caller = source(
                    0x4141_4141,
                    &[set(
                        0,
                        1,
                        if version_query { 254 } else { 0 },
                        254,
                        target.id,
                    )],
                    &[],
                    2,
                )?;
                let mut sources = vec![caller, target];
                if reverse {
                    sources.reverse();
                }
                cases.push(Case {
                    name: format!("external-{networking}-{version_query}-{reverse}"),
                    sources,
                    networking,
                });
            }
        }
        let mut target = source(0x4242_4242, &[], &[], 1)?;
        target.flags.is_static = true;
        cases.push(Case {
            name: format!("static-condition-{networking}"),
            sources: vec![
                source(
                    0x4141_4141,
                    &[
                        condition(9, 0x88, 4, 10, target.id, u32::MAX, 1),
                        set(0, 0, 255, 255, 7),
                    ],
                    &[],
                    1,
                )?,
                target,
            ],
            networking,
        });
    }
    Ok(cases)
}
