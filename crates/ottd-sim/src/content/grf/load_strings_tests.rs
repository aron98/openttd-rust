use super::load_strings::{Definition, StringKey, StringTable};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn definition(key: StringKey, language: u8, default_id: u32) -> Definition {
    Definition {
        key,
        language,
        new_scheme: true,
        default_id,
    }
}

#[test]
fn self_reference_sees_allocated_entry_before_translation() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let id = table.define(definition(key, 1, 2), |table, _| {
        Ok::<_, std::io::Error>(table.lookup(key).to_le_bytes().to_vec())
    })?;
    assert_eq!(id, 0x20000);
    assert_eq!(table.translation(id, 1)?, Some(id.to_le_bytes().as_slice()));
    Ok(())
}

#[test]
fn replacement_retains_first_default_and_allocated_id() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let first = table.define(definition(key, 3, 19), |_, _| {
        Ok::<_, std::io::Error>(b"first".to_vec())
    })?;
    let replaced = table.define(definition(key, 3, 29), |_, _| {
        Ok::<_, std::io::Error>(b"second".to_vec())
    })?;
    assert_eq!(replaced, first);
    assert_eq!(table.default_id(first)?, 19);
    assert_eq!(table.translation(first, 3)?, Some(b"second".as_slice()));
    Ok(())
}

#[test]
fn old_empty_language_mask_does_not_allocate_or_translate() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let mut request = definition(key, 0, 2);
    request.new_scheme = false;
    let mut calls = 0;
    let id = table.define(request, |_, _| {
        calls += 1;
        Ok::<_, std::io::Error>(Vec::new())
    })?;
    assert_eq!(id, 1);
    assert_eq!(calls, 0);
    assert_eq!(table.lookup(key), 2);
    Ok(())
}

#[test]
fn exact_empty_translation_beats_unspecified_fallback() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let id = table.define(definition(key, 0x7f, 2), |_, _| {
        Ok::<_, std::io::Error>(b"fallback".to_vec())
    })?;
    table.define(definition(key, 3, 2), |_, _| {
        Ok::<_, std::io::Error>(Vec::new())
    })?;
    assert_eq!(table.translation(id, 3)?, Some(b"".as_slice()));
    assert_eq!(table.translation(id, 4)?, Some(b"fallback".as_slice()));
    Ok(())
}

#[test]
fn builtin_lookup_uses_flattened_bounds_instead_of_table_count() -> Result {
    let mut bytes = vec![0; 572];
    bytes
        .get_mut(..4)
        .ok_or("magic")?
        .copy_from_slice(&0x474e_414c_u32.to_le_bytes());
    bytes
        .get_mut(4..8)
        .ok_or("version")?
        .copy_from_slice(&0x2ad1_09ab_u32.to_le_bytes());
    bytes
        .get_mut(88..90)
        .ok_or("table0")?
        .copy_from_slice(&1_u16.to_le_bytes());
    bytes
        .get_mut(90..92)
        .ok_or("table1")?
        .copy_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(b"\x01A\x01B");
    let pack = super::language_pack::BuiltinPack::new(&bytes)?;
    assert_eq!(pack.lookup(0)?, b"A");
    assert_eq!(pack.lookup(1)?, b"B");
    assert_eq!(pack.lookup(2048)?, b"B");
    assert_eq!(pack.lookup(2)?, b"(undefined string)");
    assert!(pack.lookup(26 << 11).is_err());
    assert!(pack.lookup(32 << 11).is_err());
    Ok(())
}

#[test]
fn old_mask_preserves_german_french_spanish_order() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let mut request = definition(key, 0x1c, 2);
    request.new_scheme = false;
    let mut calls = Vec::new();
    table.define(request, |_, language| {
        calls.push(language);
        Ok::<_, std::io::Error>(vec![language])
    })?;
    assert_eq!(calls, [2, 3, 4]);
    assert_eq!(table.entries.len(), 1);
    Ok(())
}

#[test]
fn english_old_mask_suppresses_other_languages() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let mut request = definition(key, 0x1f, 2);
    request.new_scheme = false;
    let mut calls = Vec::new();
    table.define(request, |_, language| {
        calls.push(language);
        Ok::<_, std::io::Error>(vec![language])
    })?;
    assert_eq!(calls, [1]);
    Ok(())
}

#[test]
fn absent_translation_follows_custom_default_then_real_bytes() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let first = table.define(definition(key, 3, 5), |_, _| {
        Ok::<_, std::io::Error>(b"French".to_vec())
    })?;
    let second = table.define(
        definition(
            StringKey {
                local_id: 0xd801,
                ..key
            },
            3,
            first,
        ),
        |_, _| Ok::<_, std::io::Error>(b"second".to_vec()),
    )?;
    let bytes = table.resolve(second, 4, |id| {
        assert_eq!(id, 5);
        Ok::<_, std::io::Error>(b"builtin bytes".as_slice())
    })?;
    assert_eq!(bytes, b"builtin bytes");
    Ok(())
}

#[test]
fn cyclic_default_is_explicit_host_refusal() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd800,
    };
    let first = table.define(definition(key, 3, 0x20000), |_, _| {
        Ok::<_, std::io::Error>(Vec::new())
    })?;
    assert!(matches!(
        table.resolve(first, 4, |_| Ok::<_, std::io::Error>(b"".as_slice())),
        Err(super::load_strings::TableError::DefaultCycle(0x20000))
    ));
    Ok(())
}

#[test]
fn real_translator_resolves_self_reference_and_callback_alias() -> Result {
    let mut table = StringTable::default();
    let key = StringKey {
        grfid: 7,
        local_id: 0xd000,
    };
    let mut budget = super::text::Budget::new(super::ScanLimits::default());
    let id = table.define(definition(key, 1, 2), |strings, _| {
        super::text_translate::translate_in(
            super::text_translate::Input {
                raw: &[0x81, 0x00, 0xd4],
                newlines: true,
                offset: 0,
                context: super::text_mapped::TextContext::default(),
            },
            &mut budget,
            |local| Some(strings.inline_id(key.grfid, local)),
        )
    })?;
    let mut expected = Vec::new();
    for code in [super::text_codes::SCC_NEWGRF_STRINL, id] {
        let (bytes, length) = super::text_reader::encode(code);
        expected.extend_from_slice(bytes.get(..length).ok_or("encoded bytes")?);
    }
    assert_eq!(table.translation(id, 1)?, Some(expected.as_slice()));
    assert_eq!(table.inline_id(8, 0xd400), 2);
    Ok(())
}

#[test]
fn native_capacity_refuses_new_key_but_updates_existing_key() -> Result {
    let mut table = StringTable::default();
    for local_id in 0..u32::try_from(super::load_strings::CAPACITY)? {
        table.define(
            definition(StringKey { grfid: 7, local_id }, 1, 2),
            |_, _| Ok::<_, std::io::Error>(b"s".to_vec()),
        )?;
    }
    let new_id = table.define(
        definition(
            StringKey {
                grfid: 8,
                local_id: 0,
            },
            1,
            2,
        ),
        |_, _| Err(std::io::Error::other("native cap must precede translation")),
    )?;
    assert_eq!(new_id, 1);
    let existing = table.define(
        definition(
            StringKey {
                grfid: 7,
                local_id: 0,
            },
            1,
            29,
        ),
        |_, _| Ok::<_, std::io::Error>(b"replacement".to_vec()),
    )?;
    assert_eq!(existing, 0x20000);
    assert_eq!(table.default_id(existing)?, 2);
    assert_eq!(
        table.translation(existing, 1)?,
        Some(b"replacement".as_slice())
    );
    assert_eq!(table.entries.len(), super::load_strings::CAPACITY);
    Ok(())
}

#[test]
fn configured_generic_action4_uses_session_table() -> Result {
    use super::{
        load::{RuntimeInputs, run_with_context},
        load_language_state::{LanguageInput, LanguageLimits},
    };
    let bytes = super::load_context_tests::grf_control_cases::encode(
        &[
            vec![8, 8, 7, 0, 0, 0, b'n', 0, b'd', 0],
            vec![4, 0, 0x81, 1, 0, 0xd8, b'T', 0],
        ],
        1,
    )?;
    let mut pack = vec![0; 572];
    pack.get_mut(..4)
        .ok_or("magic")?
        .copy_from_slice(&0x474e_414c_u32.to_le_bytes());
    pack.get_mut(4..8)
        .ok_or("version")?
        .copy_from_slice(&0x2ad1_09ab_u32.to_le_bytes());
    let packs = [pack.as_slice()];
    let input = super::LoadInput {
        name: "strings.grf",
        bytes: Some(&bytes),
        identity: super::GrfIdentity {
            grfid: 7,
            md5: [0; 16],
        },
        metadata_version: 0,
        palette: super::Palette::Windows,
        parameters: &[],
        flags: super::LoadFlags::default(),
    };
    let result = run_with_context(
        &[input],
        &[],
        super::ControlOptions::default(),
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected: 0,
                limits: LanguageLimits::default(),
            }),
        },
    );
    assert!(result.is_ok(), "generic Action4 must execute: {result:?}");
    let (_, _, report) = result?;
    let report = report.ok_or("missing language report")?;
    let entry = report.strings.first().ok_or("missing allocated string")?;
    assert_eq!(
        entry.key,
        StringKey {
            grfid: 7,
            local_id: 0xd800
        }
    );
    assert_eq!(entry.default_id, 2);
    assert_eq!(entry.translations, [(1, b"T".to_vec())]);
    Ok(())
}

#[test]
fn replacement_emission_budget_is_cumulative() -> Result {
    let mut table = StringTable::new(super::load_strings::Limits {
        entries: 1,
        definitions: 10,
        emitted_bytes: 4,
    });
    let request = definition(
        StringKey {
            grfid: 7,
            local_id: 0xd800,
        },
        1,
        2,
    );
    for _ in 0..2 {
        table.define(request, |_, _| Ok::<_, std::io::Error>(b"AB".to_vec()))?;
    }
    assert!(matches!(
        table.define(request, |_, _| Ok::<_, std::io::Error>(b"C".to_vec())),
        Err(super::load_strings::TableError::Resource(
            "translated bytes"
        ))
    ));
    assert_eq!(table.translation(0x20000, 1)?, Some(b"AB".as_slice()));
    Ok(())
}

#[test]
fn empty_replacement_still_consumes_definition_work() -> Result {
    let mut table = StringTable::new(super::load_strings::Limits {
        entries: 1,
        definitions: 1,
        emitted_bytes: 0,
    });
    let request = definition(
        StringKey {
            grfid: 7,
            local_id: 0xd800,
        },
        1,
        2,
    );
    table.define(request, |_, _| Ok::<_, std::io::Error>(Vec::new()))?;
    assert!(matches!(
        table.define(request, |_, _| Ok::<_, std::io::Error>(Vec::new())),
        Err(super::load_strings::TableError::Resource("definitions"))
    ));
    Ok(())
}

#[test]
fn host_entry_refusal_is_distinct_from_native_cap_result() {
    let mut table = StringTable::new(super::load_strings::Limits {
        entries: 0,
        definitions: 1,
        emitted_bytes: 1,
    });
    let request = definition(
        StringKey {
            grfid: 7,
            local_id: 0xd800,
        },
        1,
        2,
    );
    assert!(matches!(
        table.define(request, |_, _| Ok::<_, std::io::Error>(Vec::new())),
        Err(super::load_strings::TableError::Resource("entries"))
    ));
    assert!(table.entries.is_empty());
}
