use super::{Cases, Result, action8, binary, branch, info, parameter, text};
/// Generate the complete boundary scenarios.
/// # Errors
/// Returns malformed input, framing overflow or projection errors.
pub fn cases() -> Result<Cases> {
    let mut cases = Vec::new();
    let initial = binary(*b"VRSN", &7_u32.to_le_bytes())?;
    for (name, action) in [
        ("empty", vec![0x14, 0]),
        ("unknown-type", vec![0x14, b'X', 0, 0, 0, 0]),
        (
            "unknown-nested",
            info(&[branch(*b"WHAT", std::slice::from_ref(&initial))]),
        ),
        (
            "known-overlong",
            info(&[initial.clone(), b"BNPAR\xff\xff".to_vec()]),
        ),
        (
            "unknown-overlong",
            info(&[initial.clone(), b"BWHAT\xff\xff".to_vec()]),
        ),
        ("multiple-info", {
            let mut v = vec![0x14];
            v.extend(branch(*b"INFO", std::slice::from_ref(&initial)));
            v.extend(branch(*b"INFO", &[binary(*b"MINV", &[3, 0, 0, 0])?]));
            v.push(0);
            v
        }),
        ("trailing", {
            let mut v = info(std::slice::from_ref(&initial));
            v.extend(b"garbage");
            v
        }),
    ] {
        cases.push((format!("framing-{name}"), vec![action, action8()]));
    }
    let complete = info(&[
        initial.clone(),
        text(*b"DESC", 2, b"raw"),
        parameter(0, &[binary(*b"MASK", &[1, 2, 3])?]),
    ]);
    for end in 1..complete.len() {
        cases.push((
            format!("truncate-{end}"),
            vec![complete.get(..end).ok_or("prefix")?.to_vec(), action8()],
        ));
    }
    for tag in [
        *b"INFO", *b"NAME", *b"DESC", *b"URL_", *b"NPAR", *b"PALS", *b"BLTR", *b"VRSN", *b"MINV",
        *b"PARA", *b"TYPE", *b"LIMI", *b"MASK", *b"DFLT", *b"VALU", *b"WHAT",
    ] {
        for (kind, node) in [
            ("B", binary(tag, &[1, 2, 3, 4])?),
            ("T", text(tag, 2, b"text")),
            ("C", branch(tag, std::slice::from_ref(&initial))),
        ] {
            cases.push((
                format!("tag-{}-{kind}", std::str::from_utf8(&tag)?),
                vec![info(&[node.clone(), parameter(0, &[node])]), action8()],
            ));
        }
    }
    cases.extend(order()?);
    cases.extend(versions_palette()?);
    Ok(cases)
}
fn order() -> Result<Cases> {
    let mut cases = Vec::new();
    let initial = binary(*b"VRSN", &7_u32.to_le_bytes())?;
    for (name, actions) in [
        ("after8", vec![action8(), vec![0x14]]),
        ("missing8", vec![info(std::slice::from_ref(&initial))]),
        (
            "two14",
            vec![
                info(std::slice::from_ref(&initial)),
                info(&[binary(*b"MINV", &[1, 0, 0, 0])?]),
                action8(),
            ],
        ),
        (
            "duplicate8",
            vec![action8(), b"\x08\x08FAILsecond\0".to_vec()],
        ),
        (
            "system8",
            vec![
                info(std::slice::from_ref(&initial)),
                b"\x08\x08\xffABCname\0".to_vec(),
            ],
        ),
        (
            "zero8",
            vec![
                info(std::slice::from_ref(&initial)),
                b"\x08\x08\0\0\0\0name\0".to_vec(),
            ],
        ),
        (
            "invalid8",
            vec![info(&[initial]), b"\x08\x01TESTname\0".to_vec()],
        ),
    ] {
        cases.push((format!("order-{name}"), actions));
    }
    for skip in [
        vec![1, 0, 1, 1],
        vec![5, 0, 1],
        vec![0x0a, 1, 1, 0, 0],
        vec![0x11, 1, 0],
        vec![0x12, 1, 0, 1, 0, 0],
    ] {
        cases.push((
            format!("skip-{}", skip.first().ok_or("skip")?),
            vec![skip, vec![0x14], action8()],
        ));
    }
    Ok(cases)
}
fn versions_palette() -> Result<Cases> {
    let mut cases = Vec::new();
    for tag in [*b"VRSN", *b"MINV"] {
        for length in [0, 3, 4, 5] {
            for value in [0_u32, 1, u32::MAX] {
                let mut data = value.to_le_bytes().to_vec();
                data.resize(length, 0);
                cases.push((
                    format!("version-{}-{length}-{value}", std::str::from_utf8(&tag)?),
                    vec![
                        info(&[
                            binary(tag, &data)?,
                            binary(*b"VRSN", &[7, 0, 0, 0])?,
                            binary(tag, &data)?,
                        ]),
                        action8(),
                    ],
                ));
            }
        }
    }
    for tag in [*b"PALS", *b"BLTR"] {
        for value in [b'D', b'W', b'A', b'*', b'3', b'8', b'?'] {
            for length in [0, 1, 2] {
                cases.push((
                    format!("palette-{}-{value}-{length}", std::str::from_utf8(&tag)?),
                    vec![
                        info(&[
                            binary(*b"PALS", b"W")?,
                            binary(*b"BLTR", b"3")?,
                            binary(tag, &vec![value; length])?,
                        ]),
                        action8(),
                    ],
                ));
            }
        }
    }
    Ok(cases)
}
