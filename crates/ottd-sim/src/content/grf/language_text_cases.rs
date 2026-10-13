use super::{Pack, Result};

#[derive(serde::Serialize)]
pub(super) struct Query {
    pub id: String,
    pub stage: u8,
    pub file: usize,
    pub line: u32,
    pub grfid: u32,
    pub language: u8,
    pub newlines: bool,
    pub raw: Vec<u8>,
}

pub(super) fn choice(kind: u8, offset: u8, length: usize) -> Vec<u8> {
    let mut raw = vec![0x9a, kind];
    if kind != 0x14 {
        raw.push(offset);
    }
    for key in 1..=6_u8 {
        raw.extend([0x9a, 0x10, key]);
        raw.extend(std::iter::repeat_n(b'A'.saturating_add(key), length));
    }
    raw.extend([0x9a, 0x11, b'D', 0x9a, 0x12]);
    raw
}

pub(super) fn all(pack: &Pack, grfid: u32) -> Result<Vec<Query>> {
    let mut cases = Vec::new();
    for code in [0x0e, 0x0f] {
        for index in [0, 1, 2, 7, 8, 15, 16, 255] {
            cases.push((
                format!("direct-{code:02x}-{index}"),
                vec![0x9a, code, index],
            ));
        }
    }
    for kind in 0x13..=0x15 {
        for offset in [0x7f, 0x80, 0xff] {
            cases.push((
                format!("choice-{kind:02x}-{offset}"),
                choice(kind, offset, 1),
            ));
        }
        for length in [0, 254, 255, 256] {
            cases.push((
                format!("length-{kind:02x}-{length}"),
                choice(kind, 0x80, length),
            ));
        }
        let complete = choice(kind, 0x80, 1);
        cases.push((
            format!("unfinished-{kind:02x}"),
            complete
                .get(..complete.len().saturating_sub(2))
                .ok_or("choice")?
                .to_vec(),
        ));
        let mut duplicate = vec![0x9a, kind];
        if kind != 0x14 {
            duplicate.push(0x80);
        }
        duplicate.extend([0x9a, 0x10, 1, b'A', 0x9a, 0x10, 1, b'B', 0x9a, 0x12]);
        cases.push((format!("duplicate-no-default-{kind:02x}"), duplicate));
    }
    for length in [65_534, 65_535, 65_536] {
        cases.push((format!("word-length-{length}"), choice(0x14, 0x80, length)));
    }
    for outer in 0x13..=0x15 {
        for inner in 0x13..=0x15 {
            let mut raw = vec![0x9a, outer];
            if outer != 0x14 {
                raw.push(0x80);
            }
            raw.extend([0x9a, 0x11, b'O']);
            raw.extend(choice(inner, 0x80, 1));
            raw.extend([b'Z', 0x9a, 0x12]);
            cases.push((format!("nested-{outer:02x}-{inner:02x}"), raw));
        }
    }
    cases.push(("original-inline".into(), vec![0x81, 0x00, 0x00]));
    let mut queries = Vec::new();
    for (id, raw) in cases {
        for stage in [2, 5] {
            queries.push(Query {
                id: format!("{id}-stage{stage}"),
                stage,
                file: 0,
                line: 0,
                grfid,
                language: pack.language,
                newlines: true,
                raw: raw.clone(),
            });
        }
    }
    Ok(queries)
}
