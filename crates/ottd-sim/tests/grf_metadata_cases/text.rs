use super::{Cases, action8, branch, info, parameter, text};
/// Generate localization and Action8 interaction cases.
pub fn metadata() -> Cases {
    let mut cases = Vec::new();
    for languages in [
        vec![0, 1],
        vec![1, 0],
        vec![2, 255],
        vec![127, 0, 1],
        vec![1, 127, 0],
        vec![2, 1, 2],
    ] {
        for empty in [false, true] {
            let name = format!("localization-{languages:?}-{empty}");
            let mut nodes = Vec::new();
            for language in languages.iter().copied() {
                let raw = if empty {
                    b"".as_slice()
                } else {
                    b"raw\xff\r\n".as_slice()
                };
                nodes.extend([
                    text(*b"NAME", language, raw),
                    text(*b"DESC", language, raw),
                    text(*b"URL_", language, raw),
                    parameter(
                        0,
                        &[
                            text(*b"NAME", language, raw),
                            text(*b"DESC", language, raw),
                            branch(*b"VALU", &[text(0_u32.to_le_bytes(), language, raw)]),
                        ],
                    ),
                ]);
            }
            cases.push((name, vec![info(&nodes), action8()]));
        }
    }
    for description in [None, Some(b"".as_slice()), Some(b"new".as_slice())] {
        let mut action = action8();
        if let Some(raw) = description {
            action.extend(raw);
            action.push(0);
        }
        cases.push((
            format!("action8-description-{description:?}"),
            vec![
                info(&[
                    text(*b"NAME", 127, b"previous"),
                    text(*b"DESC", 127, b"previous"),
                ]),
                action,
            ],
        ));
    }
    cases
}

/// Generate binary translator inputs for every code family.
pub fn direct() -> Vec<(String, Vec<u8>, bool)> {
    let mut cases = Vec::new();
    for unicode in [false, true] {
        for newlines in [false, true] {
            for byte in 0_u8..=255 {
                let mut raw = if unicode {
                    vec![0xc3, 0x9e]
                } else {
                    Vec::new()
                };
                if unicode {
                    raw.extend([0xee, 0x80 | (byte >> 6), 0x80 | (byte & 63)]);
                } else {
                    raw.push(byte);
                }
                raw.extend([0x34, 0x12, b'z']);
                cases.push((format!("base-{unicode}-{newlines}-{byte}"), raw, newlines));
                let mut extended = if unicode {
                    vec![0xc3, 0x9e, 0xee, 0x82, 0x9a]
                } else {
                    vec![0x9a]
                };
                extended.extend([byte, 0x34, 0x12, b'z']);
                cases.push((
                    format!("extended-{unicode}-{newlines}-{byte}"),
                    extended,
                    newlines,
                ));
            }
        }
    }
    for length in 0..=2 {
        for code in [1, 0x1f, 0x81, 0x9a] {
            let mut raw = vec![code];
            raw.extend(vec![0x34; length]);
            cases.push((format!("operand-{code}-{length}"), raw, true));
        }
    }
    for subcode in [0, 3, 14, 15, 16, 19, 20, 21] {
        for length in 0..=2 {
            let mut raw = vec![0x9a, subcode];
            raw.extend(vec![0x34; length]);
            cases.push((format!("extended-operand-{subcode}-{length}"), raw, true));
        }
    }
    for id in (0_u16..=0x00e1)
        .chain(0x200e..=0x2042)
        .chain(0x2058..=0x205d)
        .chain(0x4801..=0x483c)
        .chain([0xcfff, 0xd000, 0xd400, 0xd7ff, 0xd800, 0xffff])
    {
        let mut raw = vec![0x81];
        raw.extend(id.to_le_bytes());
        cases.push((format!("string-id-{id}"), raw, false));
    }
    for (name,raw) in [
        ("utf8-invalid",b"\xc3\x9e\xc0\xaf\xe0\x80\x80\xf4\x90\x80\x80\xff".as_slice()),
        ("utf8-boundary",b"\xc3\x9e\xed\x9f\xbf\xed\xa0\x80\xee\x81\xbf\xee\x84\x80\xee\x87\xbf\xee\x88\x80\xf4\x8f\xbf\xbf"),
        ("embedded-nul",b"prefix\0suffix"),
        ("push-surrogate",b"\x9a\x03\0\xd8"),
        ("choice-missing",b"a\x9a\x13\x80\x9a\x10\x01other\x9a\x12z"),
        ("choice-incomplete",b"a\x9a\x15\x80\x9a\x11lost"),
        ("choice-duplicate",b"a\x9a\x14\x9a\x11one\x9a\x11two\x9a\x12z"),
        ("choice-invalid-nesting",b"a\x9a\x13\x80\x9a\x11one\x9a\x14two\x9a\x12z"),
        ("choice-nested",b"a\x9a\x14\x9a\x11case\x9a\x15\x80\x9a\x11plural\x9a\x12end\x9a\x12z"),
        ("choice-prefix",b"a\x9a\x14before\x9a\x11default\x9a\x12z"),
        ("choice-unexpected",b"a\x9a\x10\x01b\x9a\x11c\x9a\x12d"),
    ] {cases.push((name.to_owned(),raw.to_vec(),true));}
    cases
}
