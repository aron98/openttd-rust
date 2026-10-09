use super::{Case, Result, condition, encode, info, set, single, source};
pub(super) fn cases() -> Result<Vec<Case>> {
    let mut cases = vec![
        single(
            "duplicate-info-later-stages",
            &[condition(9, 0x84, 4, 2, 0, u32::MAX, 1), info(0x4141_4141)],
            &[],
            1,
        )?,
        single(
            "modify-condition",
            &[
                vec![6, 0, 1, 4, 255],
                condition(9, 0, 1, 2, 9, 255, 1),
                set(1, 0, 255, 255, 7),
            ],
            &[1],
            1,
        )?,
        single(
            "modify-substitution",
            &[
                vec![6, 0, 1, 1, 255],
                vec![6, 1, 4, 5, 255],
                set(2, 0, 255, 255, 7),
            ],
            &[255, 99],
            1,
        )?,
        single(
            "revisit-substitution",
            &[
                vec![0x10, 42],
                vec![6, 0, 0x84, 5, 255],
                set(2, 0, 255, 255, 0),
                set(1, 1, 1, 255, 1),
                condition(9, 1, 1, 4, 2, 255, 42),
            ],
            &[1, 0],
            2,
        )?,
        single(
            "undefined-final-word",
            &[vec![6, 0, 8, 1, 255], vec![0x0c, 0, 0, 0, 0, 0, 0, 0, 0]],
            &[1],
            1,
        )?,
        single(
            "undefined-middle-word",
            &[
                vec![6, 0, 12, 1, 255],
                vec![0x0c, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ],
            &[1, 2],
            1,
        )?,
        single(
            "undefined-final-127",
            &[vec![6, 0, 127, 1, 255], vec![0x0c; 128]],
            &[1; 31],
            2,
        )?,
        single("unrequested-v1-real", &[], &[], 1)?,
    ];
    let unexpected = cases
        .last_mut()
        .ok_or("unexpected case")?
        .sources
        .first_mut()
        .ok_or("source")?;
    let bytes = unexpected.bytes.as_mut().ok_or("bytes")?;
    bytes.truncate(bytes.len().saturating_sub(2));
    bytes.extend([12, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 0, 0]);
    let mut next_real = source(0x4141_4141, &[], &[0], 1)?;
    let mut bytes = encode(
        &[
            info(next_real.id),
            vec![6, 0, 1, 3, 255],
            vec![1, 0, 1, 2],
            vec![6],
        ],
        1,
    )?;
    bytes.truncate(bytes.len().saturating_sub(2));
    bytes.extend([12, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 0, 0]);
    next_real.bytes = Some(bytes);
    cases.push(Case {
        name: "substitution-next-real-early-return".into(),
        sources: vec![next_real],
        networking: false,
    });
    cases.push(empty_override()?);
    cases.push(reserve_guard()?);
    Ok(cases)
}
fn empty_override() -> Result<Case> {
    let mut empty = source(0x4141_4141, &[vec![6, 255]], &[], 1)?;
    empty
        .bytes
        .as_mut()
        .ok_or("empty lookahead bytes")?
        .push(255);
    Ok(Case {
        name: "empty-override-lookahead".into(),
        sources: vec![empty],
        networking: false,
    })
}
fn reserve_guard() -> Result<Case> {
    let mut file = source(0x4141_4141, &[], &[], 1)?;
    file.bytes = Some(encode(
        &[condition(9, 0x84, 4, 2, 0, u32::MAX, 1), info(file.id)],
        1,
    )?);
    Ok(Case {
        name: "unknown-reserve-entry-guard".into(),
        sources: vec![file],
        networking: false,
    })
}
