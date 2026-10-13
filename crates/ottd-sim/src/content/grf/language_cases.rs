use super::{Pack, Result, mapping, programs, text_cases};
use std::path::Path;

pub(super) struct Case {
    pub name: String,
    pub packs: Vec<Vec<u8>>,
    pub selected: u8,
    pub sources: Vec<programs::Source>,
    pub queries: Vec<text_cases::Query>,
}

pub(super) fn base(name: String, bytes: &[u8], records: &[Vec<u8>]) -> Result<Case> {
    let pack = Pack::header(bytes)?;
    let source = programs::source(0x4141_4141, records, &[], 1)?;
    let mut queries = Vec::new();
    for stage in [2, 3, 4, 5] {
        for (kind, raw) in [
            ("direct", vec![0x9a, 0x0e, 1, 0x9a, 0x0f, 1]),
            ("gender", text_cases::choice(0x13, 0x80, 1)),
            ("case", text_cases::choice(0x14, 0x80, 1)),
            ("plural", text_cases::choice(0x15, 0x80, 1)),
        ] {
            queries.push(text_cases::Query {
                id: format!("{kind}-{stage}"),
                stage,
                file: 0,
                line: 0,
                grfid: source.id,
                language: pack.language,
                newlines: true,
                raw,
            });
        }
    }
    Ok(Case {
        name,
        packs: vec![bytes.to_vec()],
        selected: pack.language,
        sources: vec![source],
        queries,
    })
}

pub(super) fn records(pack: &Pack) -> Result<Vec<Vec<u8>>> {
    Ok(vec![
        mapping(pack, true)?,
        mapping(pack, false)?,
        vec![0, 8, 1, 1, pack.language, 0x15, pack.plural],
    ])
}

fn catalogs(bytes: &[u8]) -> Result<Vec<Case>> {
    let pack = Pack::header(bytes)?;
    let mut cases = Vec::new();
    for (name, offset, value) in [
        ("magic", 0, 0),
        ("version", 4, 0),
        ("direction", 179, 2),
        ("language127", 182, 127),
        ("language255", 182, 255),
        ("gender8", 183, 8),
        ("case16", 184, 16),
        ("plural15", 178, 15),
    ] {
        let mut invalid = bytes.to_vec();
        *invalid.get_mut(offset).ok_or("header offset")? = value;
        let mut case = base(format!("catalog-{name}"), bytes, &records(&pack)?)?;
        case.packs.insert(0, invalid);
        cases.push(case);
    }
    for (name, cut) in [("empty", 0), ("short-header", 571)] {
        let mut case = base(format!("catalog-{name}"), bytes, &[])?;
        case.packs
            .insert(0, bytes.get(..cut).ok_or("header cut")?.to_vec());
        cases.push(case);
    }
    for reverse in [false, true] {
        let mut changed = bytes.to_vec();
        *changed.get_mut(183).ok_or("gender count")? = 0;
        let mut case = base(
            format!("catalog-duplicate-{reverse}"),
            bytes,
            &records(&pack)?,
        )?;
        case.packs.push(changed);
        if reverse {
            case.packs.reverse();
        }
        cases.push(case);
    }
    Ok(cases)
}

fn properties(bytes: &[u8]) -> Result<Vec<Case>> {
    let pack = Pack::header(bytes)?;
    let mut cases = Vec::new();
    for plural in 0..=16 {
        cases.push(base(
            format!("plural-{plural}"),
            bytes,
            &[
                vec![0, 8, 1, 1, pack.language, 0x15, 3],
                vec![0, 8, 1, 1, pack.language, 0x15, plural],
            ],
        )?);
    }
    for language in [0, 126, 127, 128, 255] {
        for property in [0x13, 0x14, 0x15] {
            let mut record = vec![0, 8, 1, 1, language];
            if language == 255 {
                record.extend([255, 0]);
            }
            record.push(property);
            if property == 0x15 {
                record.push(2);
            } else {
                record.extend([1, b'x', 0, 2, 0, 0]);
            }
            cases.push(base(
                format!("language-{language}-property-{property}"),
                bytes,
                &[record],
            )?);
        }
    }
    for (name, record) in [
        ("count-zero", vec![0, 8, 1, 0, pack.language, 0x13]),
        ("zero-properties", vec![0, 8, 0, 1, pack.language]),
        (
            "extended-crossing",
            vec![0, 8, 1, 2, 255, 126, 0, 0x15, 1, 2],
        ),
        (
            "extended-65535",
            vec![0, 8, 1, 2, 255, 255, 255, 0x15, 1, 2],
        ),
        ("unknown-feature", vec![0, 255, 0, 0, 0]),
        ("signal-feature", vec![0, 14, 0, 0, 0]),
        ("unknown-feature-header-truncated", vec![0, 255]),
        ("signal-feature-header-truncated", vec![0, 14]),
        (
            "utf8-empty",
            vec![0, 8, 1, 1, pack.language, 0x13, 1, 0xc3, 0x9e, 0, 0],
        ),
        (
            "missing-name",
            vec![0, 8, 1, 1, pack.language, 0x14, 1, b'x', 0, 0],
        ),
        (
            "unread-tail",
            vec![0, 8, 1, 1, pack.language, 0x15, 1, 0xff, 0xff],
        ),
    ] {
        cases.push(base(name.into(), bytes, &[record])?);
    }
    for property in [0x13, 0x14, 0x15] {
        let record = if property == 0x15 {
            vec![0, 8, 1, 1, pack.language, property, 2]
        } else {
            vec![0, 8, 1, 1, pack.language, property, 1, b'x', 0, 0]
        };
        for end in 1..record.len() {
            cases.push(base(
                format!("truncated-{property}-{end}"),
                bytes,
                &[record.get(..end).ok_or("record cut")?.to_vec()],
            )?);
        }
    }
    Ok(cases)
}

pub(super) fn all(directory: &Path) -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for name in ["english", "german", "czech", "russian"] {
        let bytes = std::fs::read(directory.join(format!("{name}.lng")))?;
        let pack = Pack::header(&bytes)?;
        let mut case = base(name.into(), &bytes, &records(&pack)?)?;
        case.queries = text_cases::all(&pack, 0x4141_4141)?;
        cases.push(case);
    }
    let czech = std::fs::read(directory.join("czech.lng"))?;
    cases.extend(catalogs(&czech)?);
    cases.extend(properties(&czech)?);
    cases.extend(super::bodies::all(&czech)?);
    cases.extend(super::ordered::all(directory, &czech)?);
    Ok(cases)
}
