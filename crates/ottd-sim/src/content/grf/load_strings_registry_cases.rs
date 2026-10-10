use super::{Result, action4, programs};

fn translator(target: u32) -> Result<programs::Source> {
    let mut record = vec![0x13];
    record.extend(target.to_le_bytes());
    record.extend([1, 1, 0, 0xd8, b'T', 0]);
    programs::source(8, &[record], &[], 1)
}

pub(super) fn cases() -> Result<Vec<programs::Case>> {
    let mut cases = Vec::new();
    for missing in [true, false] {
        for inactive_first in [true, false] {
            let mut active = programs::source(7, &[action4(0, 1, 0xd800, 1, b"A")], &[], 1)?;
            active.name = "active.grf".into();
            let mut inactive = programs::source(7, &[], &[], 1)?;
            inactive.name = "inactive.grf".into();
            if missing {
                inactive.bytes = None;
            } else {
                inactive.bytes = Some(programs::encode(
                    &[programs::info(9), programs::info(9)],
                    1,
                )?);
            }
            let sources = if inactive_first {
                vec![inactive, active, translator(7)?]
            } else {
                vec![active, inactive, translator(7)?]
            };
            cases.push(programs::Case {
                name: format!(
                    "translation-first-config-missing-{missing}-inactive-first-{inactive_first}"
                ),
                sources,
                networking: false,
            });
        }
    }
    for alias_first in [false, true] {
        let active = programs::source(7, &[action4(0, 1, 0xd800, 1, b"A")], &[11], 1)?;
        let mut alias = programs::source(9, &[], &[99], 1)?;
        alias.name.clone_from(&active.name);
        alias.bytes.clone_from(&active.bytes);
        alias.flags.init_only = true;
        let sources = if alias_first {
            vec![alias, active, translator(9)?]
        } else {
            vec![active, alias, translator(9)?]
        };
        cases.push(programs::Case {
            name: format!("translation-filename-alias-init-only-first-{alias_first}"),
            sources,
            networking: false,
        });
    }
    let mut target = programs::source(7, &[], &[], 1)?;
    target.flags.init_only = true;
    cases.push(programs::Case {
        name: "translation-init-only-target-after-translator".into(),
        sources: vec![translator(7)?, target],
        networking: false,
    });
    Ok(cases)
}
