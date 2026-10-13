use super::SafetyConfig;

pub(super) struct Case {
    pub id: String,
    pub bytes: Vec<u8>,
    pub configs: Vec<SafetyConfig>,
    pub is_static: bool,
}
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn encode(actions: &[Vec<u8>], version: u8) -> Result<Vec<u8>> {
    let mut bytes = if version == 1 {
        Vec::new()
    } else {
        b"\0\0GRF\x82\r\n\x1a\n\0\0\0\0\0".to_vec()
    };
    for action in std::iter::once(vec![0; 4]).chain(actions.iter().cloned()) {
        if version == 1 {
            bytes.extend(u16::try_from(action.len())?.to_le_bytes());
        } else {
            bytes.extend(u32::try_from(action.len())?.to_le_bytes());
        }
        bytes.push(255);
        bytes.extend(action);
    }
    bytes.extend(vec![0; if version == 1 { 2 } else { 4 }]);
    if version == 2 {
        let offset = u32::try_from(bytes.len().checked_sub(14).ok_or("offset")?)?;
        bytes
            .get_mut(10..14)
            .ok_or("offset field")?
            .copy_from_slice(&offset.to_le_bytes());
        bytes.extend([0; 4]);
    }
    Ok(bytes)
}
fn info(id: u32) -> Vec<u8> {
    let mut result = vec![8, 8];
    result.extend(id.to_le_bytes());
    result.extend(b"safety\0description\0");
    result
}
fn add(
    cases: &mut Vec<Case>,
    name: &str,
    actions: &[Vec<u8>],
    configs: &[SafetyConfig],
) -> Result<()> {
    for version in [1, 2] {
        let mut records = vec![info(0x1020_3040)];
        records.extend_from_slice(actions);
        cases.push(Case {
            id: format!("{name}-v{version}"),
            bytes: encode(&records, version)?,
            configs: configs.to_vec(),
            is_static: true,
        });
    }
    Ok(())
}

pub(super) fn cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for action in (0..=0x15).chain([0xfe, 0xff]) {
        add(
            &mut cases,
            &format!("action-{action:02x}"),
            &[vec![action]],
            &[],
        )?;
    }
    for target in 0..=255 {
        add(
            &mut cases,
            &format!("parameter-{target:02x}"),
            &[vec![0x0d, target]],
            &[],
        )?;
    }
    for feature in 0..=22 {
        for count in [0, 1, 2] {
            add(
                &mut cases,
                &format!("property-{feature:02x}-{count}"),
                &[vec![0, feature, count, 1, 0, 0x0d]],
                &[],
            )?;
        }
    }
    cases.extend(registry_cases()?);
    cases.extend(skip_cases()?);
    cases.extend(admission_cases()?);
    cases.extend(framing_cases()?);
    cases.extend(boundary_cases()?);
    cases.extend(inline_skip_cases()?);
    cases.extend(early_skip_cases()?);
    Ok(cases)
}

fn registry_cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for (name, sources) in [
        ("missing", vec![]),
        (
            "static",
            vec![SafetyConfig {
                grfid: 2,
                is_static: true,
            }],
        ),
        (
            "nonstatic",
            vec![SafetyConfig {
                grfid: 2,
                is_static: false,
            }],
        ),
        (
            "static-first",
            vec![
                SafetyConfig {
                    grfid: 2,
                    is_static: true,
                },
                SafetyConfig {
                    grfid: 2,
                    is_static: false,
                },
            ],
        ),
        (
            "nonstatic-first",
            vec![
                SafetyConfig {
                    grfid: 2,
                    is_static: false,
                },
                SafetyConfig {
                    grfid: 2,
                    is_static: true,
                },
            ],
        ),
    ] {
        for count in [0, 1, 2] {
            let mut action = vec![0, 8, 1, count, 255, 0, 1, 0x11];
            action.extend([2, 0, 0, 0, 3, 0, 0, 0]);
            add(
                &mut cases,
                &format!("mapping-{name}-{count}"),
                &[action],
                &sources,
            )?;
        }
    }
    let self_id = 0x1020_3040_u32;
    for (name, ids) in [
        ("none", vec![]),
        ("self", vec![self_id]),
        ("repeat-self", vec![self_id, self_id]),
        ("foreign", vec![2]),
        ("foreign-first", vec![2, self_id]),
        ("foreign-last", vec![self_id, 2]),
    ] {
        let mut action = vec![0x0e, u8::try_from(ids.len())?];
        for id in ids {
            action.extend(id.to_le_bytes());
        }
        add(&mut cases, &format!("inhibit-{name}"), &[action], &[])?;
    }
    Ok(cases)
}

fn skip_cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    let reads = [
        vec![0, 8, 1, 1, 255, 0, 1, 0x11, 2, 0, 0, 0, 3, 0, 0, 0],
        vec![1, 0, 0, 255, 1, 0, 255, 2, 0, 255, 1, 0],
        vec![5, 128, 255, 1, 0],
        vec![0x0a, 1, 1, 0, 0],
        vec![0x0d, 127],
        vec![0x0e, 1, 0x40, 0x30, 0x20, 0x10],
        vec![0x12, 1, 0, 1, 0, 0],
    ];
    for action in &reads {
        for length in 1..action.len() {
            add(
                &mut cases,
                &format!("truncated-{:02x}-{length}", action.first().ok_or("action")?),
                &[action.get(..length).ok_or("prefix")?.to_vec()],
                &[],
            )?;
        }
    }
    for (name, action) in [
        ("one-zero", vec![1, 0, 0, 0]),
        ("one-one", vec![1, 0, 1, 1]),
        ("one-multiple", vec![1, 0, 2, 1]),
        ("one-extended", vec![1, 0, 0, 255, 1, 0, 255, 2, 0, 1]),
        (
            "one-overflow",
            vec![1, 0, 0, 0, 255, 255, 255, 255, 255, 255],
        ),
        ("five-zero", vec![5, 128, 0]),
        ("five-one", vec![5, 128, 1]),
        ("five-offset-unread", vec![5, 128, 255, 2, 0, 255]),
        ("a-zero", vec![0x0a, 0]),
        ("a-multiple", vec![0x0a, 2, 1, 0, 0, 1, 0, 0]),
        ("glyph-zero", vec![0x12, 0]),
        ("glyph-multiple", vec![0x12, 2, 0, 1, 0, 0, 0, 1, 0, 0]),
    ] {
        add(
            &mut cases,
            &format!("skip-{name}"),
            &[action, vec![3], vec![0x0f], vec![0x11]],
            &[],
        )?;
    }
    Ok(cases)
}

fn admission_cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    let self_id = 0x1020_3040_u32;
    for (name, id) in [("zero", 0), ("system", 255)] {
        for version in [1, 2] {
            cases.push(Case {
                id: format!("admission-{name}-v{version}"),
                bytes: encode(&[info(id), vec![3]], version)?,
                configs: vec![],
                is_static: true,
            });
        }
    }
    cases.push(Case {
        id: "admission-nonstatic".into(),
        bytes: encode(&[info(self_id), vec![3]], 1)?,
        configs: vec![],
        is_static: false,
    });
    cases.push(Case {
        id: "admission-no-action8".into(),
        bytes: encode(&[vec![3]], 1)?,
        configs: vec![],
        is_static: true,
    });
    cases.push(Case {
        id: "contract-speed".into(),
        bytes: include_bytes!("../../../../../fixtures/content/contract-speed.grf").to_vec(),
        configs: vec![],
        is_static: true,
    });
    Ok(cases)
}

fn framing_cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    let self_id = 0x1020_3040_u32;
    for version in [1, 2] {
        for requested in [false, true] {
            for compressed in [false, true] {
                let mut actions = vec![info(self_id)];
                if requested {
                    actions.push(vec![5, 0, 1]);
                }
                let mut bytes = encode(&actions, version)?;
                bytes.truncate(
                    bytes
                        .len()
                        .checked_sub(if version == 1 { 2 } else { 8 })
                        .ok_or("end")?,
                );
                if version == 1 {
                    bytes.extend(9_u16.to_le_bytes());
                } else {
                    bytes.extend(9_u32.to_le_bytes());
                }
                bytes.push(if compressed { 0 } else { 2 });
                bytes.extend([0; 7]);
                if compressed {
                    bytes.push(1);
                }
                bytes.push(42);
                append_unsafe_end(&mut bytes, version)?;
                cases.push(Case {
                    id: format!("real-{requested}-{compressed}-v{version}"),
                    bytes,
                    configs: vec![],
                    is_static: true,
                });
            }
        }
        let mut actions = vec![info(self_id), vec![8, 8]];
        actions
            .last_mut()
            .ok_or("last")?
            .extend(2_u32.to_le_bytes());
        actions.last_mut().ok_or("last")?.extend(b"other\0");
        let mut inhibit = vec![0x0e, 1];
        inhibit.extend(self_id.to_le_bytes());
        actions.push(inhibit);
        cases.push(Case {
            id: format!("inhibit-identity-before-later-action8-v{version}"),
            bytes: encode(&actions, version)?,
            configs: vec![],
            is_static: true,
        });
        for declared in [1, 9] {
            let mut action = info(self_id);
            *action.get_mut(1).ok_or("version")? = declared;
            cases.push(Case {
                id: format!("invalid-action8-{declared}-v{version}"),
                bytes: encode(&[action, vec![2]], version)?,
                configs: vec![],
                is_static: true,
            });
        }
    }
    Ok(cases)
}

fn boundary_cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    let self_id = 0x1020_3040_u32;
    let mut oversized = encode(&[info(self_id)], 2)?;
    oversized.truncate(oversized.len().checked_sub(8).ok_or("end")?);
    oversized.extend(1_048_577_u32.to_le_bytes());
    oversized.push(255);
    append_unsafe_end(&mut oversized, 2)?;
    cases.push(Case {
        id: "oversized-pseudo-before-payload".into(),
        bytes: oversized,
        configs: vec![],
        is_static: true,
    });
    let mut stopped = encode(&[info(self_id), vec![3]], 1)?;
    stopped.truncate(stopped.len().checked_sub(2).ok_or("end")?);
    stopped.push(255);
    cases.push(Case {
        id: "unsafe-before-truncated-record".into(),
        bytes: stopped,
        configs: vec![],
        is_static: true,
    });
    for requested in [false, true] {
        let mut actions = vec![info(self_id)];
        if requested {
            actions.push(vec![5, 0, 1]);
        }
        let mut bytes = encode(&actions, 2)?;
        bytes.truncate(bytes.len().checked_sub(8).ok_or("end")?);
        bytes.extend(4_u32.to_le_bytes());
        bytes.push(253);
        bytes.extend(1_u32.to_le_bytes());
        append_unsafe_end(&mut bytes, 2)?;
        cases.push(Case {
            id: format!("sprite-reference-{requested}"),
            bytes,
            configs: vec![],
            is_static: true,
        });
    }
    Ok(cases)
}

fn append_unsafe_end(bytes: &mut Vec<u8>, version: u8) -> Result<()> {
    if version == 1 {
        bytes.extend(1_u16.to_le_bytes());
    } else {
        bytes.extend(1_u32.to_le_bytes());
    }
    bytes.extend([255, 3]);
    bytes.extend(vec![0; if version == 1 { 2 } else { 4 }]);
    if version == 2 {
        let offset = u32::try_from(bytes.len().checked_sub(14).ok_or("offset")?)?;
        bytes
            .get_mut(10..14)
            .ok_or("offset field")?
            .copy_from_slice(&offset.to_le_bytes());
        bytes.extend([0; 4]);
    }
    Ok(())
}

fn inline_skip_cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    let mut wrapped_reference = vec![240, 0];
    for _ in 0..511 {
        wrapped_reference.push(0);
        wrapped_reference.extend(std::iter::repeat_n(42, 128));
    }
    wrapped_reference.push(127);
    wrapped_reference.extend(std::iter::repeat_n(42, 127));
    for version in [1, 2] {
        for (name, length, flags, pixels) in [
            ("literal-overrun", 9_u32, 0, vec![2]),
            ("back-reference", 9, 0, vec![248, 0]),
            ("back-reference-wrap", 9, 0, wrapped_reference.clone()),
            ("length-underflow", 7, 2, vec![42; 65535]),
            (
                "zero-literal",
                136,
                0,
                std::iter::once(0)
                    .chain(std::iter::repeat_n(42, 128))
                    .collect(),
            ),
            ("length-truncation", 65545, 2, vec![42]),
            ("length-truncation-zero", 65544, 2, vec![]),
        ] {
            if version == 1 && length > u32::from(u16::MAX) {
                continue;
            }
            let mut bytes = encode(&[info(0x1020_3040), vec![5, 0, 1]], version)?;
            bytes.truncate(
                bytes
                    .len()
                    .checked_sub(if version == 1 { 2 } else { 8 })
                    .ok_or("end")?,
            );
            if version == 1 {
                bytes.extend(u16::try_from(length)?.to_le_bytes());
            } else {
                bytes.extend(length.to_le_bytes());
            }
            bytes.push(flags);
            bytes.extend([0; 7]);
            bytes.extend(pixels);
            append_unsafe_end(&mut bytes, version)?;
            cases.push(Case {
                id: format!("inline-{name}-v{version}"),
                bytes,
                configs: vec![],
                is_static: true,
            });
        }
    }
    Ok(cases)
}

fn early_skip_cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for (name, length, flags, pixels) in [
        ("literal-overrun", 9_u32, 0, vec![2]),
        ("length-truncation", 65545, 2, vec![42]),
    ] {
        let mut bytes = encode(&[vec![5, 0, 1]], 2)?;
        bytes.truncate(bytes.len().checked_sub(8).ok_or("end")?);
        bytes.extend(length.to_le_bytes());
        bytes.push(flags);
        bytes.extend([0; 7]);
        bytes.extend(pixels);
        let metadata = info(0x1020_3040);
        bytes.extend(u32::try_from(metadata.len())?.to_le_bytes());
        bytes.push(255);
        bytes.extend(metadata);
        append_unsafe_end(&mut bytes, 2)?;
        cases.push(Case {
            id: format!("filescan-inline-{name}"),
            bytes,
            configs: vec![],
            is_static: true,
        });
    }
    Ok(cases)
}
