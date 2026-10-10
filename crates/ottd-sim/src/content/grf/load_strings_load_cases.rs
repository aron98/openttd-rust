use super::programs;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "load_strings_registry_cases.rs"]
mod registry;

fn program(name: &str, id: u32, version: u8, records: &[Vec<u8>]) -> Result<programs::Case> {
    let mut first = programs::info(id);
    *first.get_mut(1).ok_or("version")? = version;
    let records = std::iter::once(first)
        .chain(records.iter().cloned())
        .collect::<Vec<_>>();
    let mut source = programs::source(id, &[], &[], 1)?;
    source.bytes = Some(programs::encode(&records, 1)?);
    Ok(programs::Case {
        name: name.into(),
        sources: vec![source],
        networking: false,
    })
}

fn action4(feature: u8, language: u8, id: u16, count: u8, raw: &[u8]) -> Vec<u8> {
    let mut bytes = vec![4, feature, language | 0x80, count];
    bytes.extend(id.to_le_bytes());
    bytes.extend(raw);
    bytes.push(0);
    bytes
}

fn features() -> Result<Vec<programs::Case>> {
    let mut cases = Vec::new();
    for version in [6, 7, 8] {
        let mut records = Vec::new();
        for feature in (0..22).chain([0x48]) {
            for id in [
                0, 0xd000, 0xd3ff, 0xd400, 0xd7ff, 0xd800, 0xdfff, 0xe000, 0xfffe, 0xffff,
            ] {
                for language in [0, 1, 3, 0x7f] {
                    records.push(action4(
                        feature,
                        language,
                        id,
                        1,
                        format!("v{version}f{feature}i{id}l{language}").as_bytes(),
                    ));
                }
            }
        }
        cases.push(program(
            &format!("generic-features-version-{version}"),
            7,
            version,
            &records,
        )?);
    }
    let mut noops = Vec::new();
    for feature in 0..=255 {
        if feature >= 22 && feature != 0x48 {
            noops.push(vec![4, feature]);
        }
    }
    for feature in 0..22 {
        noops.push(vec![4, feature, 1, 0, 0]);
        if feature > 3 && feature != 21 {
            noops.push(vec![4, feature, 1, 1, 7, b'N', 0]);
        }
    }
    noops.push(action4(0, 1, 0xfffe, 2, b"wrap"));
    noops.push(action4(0, 1, 0xd800, 0, b"zero"));
    for feature in [0, 1, 2, 3, 21] {
        for offset in [0_u16, 255, u16::MAX] {
            let mut bytes = vec![4, feature, 1, 0, 255];
            bytes.extend(offset.to_le_bytes());
            noops.push(bytes);
        }
    }
    cases.push(program("invalid-features-and-noops", 7, 8, &noops)?);
    let mut sources = Vec::new();
    let full = action4(0, 1, 0xd800, 1, b"truncated");
    for length in 1..=full.len() {
        sources.push(programs::source(
            u32::try_from(length)?.saturating_add(100),
            &[vec![0x10, 42], full.get(..length).ok_or("prefix")?.to_vec()],
            &[],
            1,
        )?);
    }
    cases.push(programs::Case {
        name: "action4-lazy-truncation".into(),
        sources,
        networking: false,
    });
    Ok(cases)
}

pub(super) fn error_order() -> Result<programs::Case> {
    Ok(programs::Case {
        name: "load-after-order".into(),
        networking: false,
        sources: vec![
            programs::source(
                8,
                &[
                    vec![0x10, 42],
                    action4(0, 1, 0xd800, 1, b"retained"),
                    vec![0x13, 7, 0, 0, 0],
                ],
                &[],
                1,
            )?,
            programs::source(7, &[], &[], 1)?,
        ],
    })
}

fn translations() -> Result<Vec<programs::Case>> {
    let mut cases = vec![error_order()?];
    for version in [7, 8] {
        let target = programs::source(7, &[action4(0, 1, 0xd800, 1, b"target")], &[], 1)?;
        let mut records = Vec::new();
        for first in [
            0xcfff_u16, 0xd000, 0xd3ff, 0xd400, 0xd7ff, 0xd800, 0xdfff, 0xe000, 0xffff,
        ] {
            for count in [0_u8, 1, 2, 255] {
                let mut bytes = vec![0x13, 7, 0, 0, 0];
                if version >= 8 {
                    bytes.push(1);
                }
                bytes.push(count);
                bytes.extend(first.to_le_bytes());
                bytes.extend(b"translated\0\0third\0");
                records.push(bytes);
            }
        }
        let mut case = program(
            &format!("translation-spans-version-{version}"),
            8,
            version,
            &records,
        )?;
        case.sources.insert(0, target);
        cases.push(case);
    }
    for variant in [
        "absent",
        "not-found",
        "disabled",
        "unknown",
        "static",
        "init-only",
    ] {
        let translator = programs::source(8, &[vec![0x13, 7, 0, 0, 0]], &[], 1)?;
        let mut target = programs::source(7, &[], &[], 1)?;
        match variant {
            "absent" => (),
            "not-found" => target.bytes = None,
            "disabled" => target = programs::source(7, &[programs::info(7)], &[], 1)?,
            "unknown" => target.bytes = Some(programs::encode(&[vec![0x0c]], 1)?),
            "static" => target.flags.is_static = true,
            "init-only" => target.flags.init_only = true,
            _ => return Err("unknown case".into()),
        }
        let mut sources = if variant == "absent" {
            Vec::new()
        } else {
            vec![target]
        };
        sources.push(translator);
        cases.push(programs::Case {
            name: format!("translation-target-{variant}"),
            sources,
            networking: false,
        });
    }
    cases.push(program(
        "self-translator",
        7,
        8,
        &[vec![0x13, 7, 0, 0, 0, 1, 1, 0, 0xd8, b'S', 0]],
    )?);
    cases.push(program(
        "inline-and-overwrite",
        7,
        8,
        &[
            action4(0, 1, 0xd000, 1, &[0x81, 0, 0xd4]),
            action4(0, 1, 0xd800, 1, &[0x81, 1, 0xd8]),
            action4(0, 1, 0xd801, 1, b"future"),
            action4(0, 1, 0xd800, 1, b""),
            vec![0x13, 7, 0, 0, 0, 1, 1, 0, 0xd8, 0],
        ],
    )?);
    Ok(cases)
}

pub(super) fn cases() -> Result<Vec<programs::Case>> {
    let mut cases = features()?;
    cases.extend(translations()?);
    cases.extend(registry::cases()?);
    Ok(cases)
}

pub(super) fn registry_cases() -> Result<Vec<programs::Case>> {
    let mut cases = registry::cases()?;
    cases.extend(
        features()?
            .into_iter()
            .filter(|case| case.name == "invalid-features-and-noops"),
    );
    Ok(cases)
}

pub(super) fn mapped(pack: &super::Pack) -> Result<programs::Case> {
    let mut mapping = vec![0, 8, 1, 1, pack.language, 0x13];
    mapping.push(1);
    mapping.extend(pack.genders.first().ok_or("mapped pack gender")?);
    mapping.extend([0, 0]);
    let raw = [0x9a, 0x0e, 1, 0x81, 0, 0xd8];
    let target = programs::source(
        7,
        &[
            action4(0, pack.language, 0xd800, 1, &raw),
            mapping.clone(),
            action4(0, pack.language, 0xd801, 1, &raw),
        ],
        &[],
        1,
    )?;
    let mut translation = vec![0x13, 7, 0, 0, 0, pack.language, 1, 2, 0xd8];
    translation.extend(raw);
    translation.push(0);
    let mut other_mapping = vec![0, 8, 1, 1, pack.language, 0x13, 1];
    other_mapping.extend(pack.genders.get(1).ok_or("second mapped pack gender")?);
    other_mapping.extend([0, 0]);
    let translator = programs::source(8, &[other_mapping, translation], &[], 1)?;
    Ok(programs::Case {
        name: "mapped-definition-and-target-context".into(),
        sources: vec![target, translator],
        networking: false,
    })
}
