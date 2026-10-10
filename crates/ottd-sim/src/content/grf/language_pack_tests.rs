use super::{language_pack::Pack, load_language::LanguageMap};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn header() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x474e_414c_u32.to_le_bytes());
    bytes.extend_from_slice(&0x2ad1_09ab_u32.to_le_bytes());
    bytes.resize(182, 0);
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
    bytes.resize(572, 0);
    bytes
}

#[test]
fn native_name_lookup_searches_inactive_fixed_slots() -> Result {
    let pack = Pack::header(&header())?;
    assert_eq!(pack.gender_count, 0);
    assert_eq!(pack.name_index(b"", true), Some(0));
    assert_eq!(pack.name_index(b"missing", true), None);
    Ok(())
}

#[test]
fn native_header_rejects_exact_maximum_gender_count() -> Result {
    let mut bytes = header();
    *bytes.get_mut(183).ok_or("missing gender count")? = 8;
    assert!(Pack::header(&bytes).is_err());
    Ok(())
}

#[test]
fn map_preserves_distinct_first_forward_and_reverse_matches() {
    let map = LanguageMap {
        genders: vec![(9, 1), (9, 0), (8, 1)],
        ..LanguageMap::default()
    };
    assert_eq!(map.forward(9, true), Some(1));
    assert_eq!(map.reverse(1, true), Some(9));
    assert_eq!(map.reverse(0, true), Some(9));
    assert_eq!(map.forward(8, true), Some(1));
}

fn translate(
    raw: &[u8],
    map: Option<&LanguageMap>,
) -> std::result::Result<Vec<u8>, super::ScanError> {
    let input = super::text_translate::Input {
        raw,
        newlines: true,
        offset: 0,
        context: super::text_mapped::TextContext {
            map,
            genders: 2,
            cases: 1,
        },
    };
    super::text_translate::translate_in(
        input,
        &mut super::text::Budget::new(super::ScanLimits::default()),
        super::string_ids::original,
    )
}

#[test]
fn mapped_controls_emit_native_indices_and_case_offset() -> Result {
    let map = LanguageMap {
        genders: vec![(7, 1)],
        cases: vec![(9, 0)],
        plural: 3,
    };
    assert_eq!(
        translate(&[0x9a, 0x0e, 7, 0x9a, 0x0f, 9], Some(&map))?,
        [0xee, 0x80, 0xbe, 1, 0xee, 0x81, 0x80, 1]
    );
    assert!(translate(&[0x9a, 0x0e, 7], None)?.is_empty());
    Ok(())
}

#[test]
fn mapped_case_list_uses_little_endian_lengths_and_default() -> Result {
    let map = LanguageMap {
        cases: vec![(9, 0)],
        ..LanguageMap::default()
    };
    let raw = [
        0x9a, 0x14, 0x9a, 0x10, 9, b'X', 0x9a, 0x11, b'D', 0x9a, 0x12,
    ];
    assert_eq!(
        translate(&raw, Some(&map))?,
        [0xee, 0x81, 0x81, 1, 1, 1, 0, b'X', 1, 0, b'D']
    );
    assert_eq!(translate(&raw, None)?, b"D");
    Ok(())
}

#[test]
fn mapped_gender_list_uses_first_reverse_mapping_and_fallback() -> Result {
    let map = LanguageMap {
        genders: vec![(7, 1), (8, 1)],
        ..LanguageMap::default()
    };
    let raw = [
        0x9a, 0x13, 0x81, 0x9a, 0x10, 7, b'G', 0x9a, 0x10, 8, b'X', 0x9a, 0x11, b'D', 0x9a, 0x12,
    ];
    assert_eq!(
        translate(&raw, Some(&map))?,
        [0xee, 0x80, 0xbd, 1, 2, 1, 1, b'D', b'G']
    );
    Ok(())
}

#[test]
fn contextual_custom_inline_refuses_while_fresh_registry_stays_undefined() -> Result {
    let raw = [0x81, 0x00, 0xd0];
    assert!(matches!(
        translate(&raw, None),
        Err(super::ScanError::UnsupportedInline { id: 0xd000 })
    ));
    assert_eq!(
        super::translate_fresh_text(&raw, true, 100)?,
        [0xee, 0x81, 0xb4, 2]
    );
    Ok(())
}

#[test]
fn activation_appends_maps_but_reserve_only_consumes_and_invalid_plural_preserves() -> Result {
    use super::{
        load::{RuntimeInputs, run_with_context},
        load_context_tests::grf_control_cases::source,
        load_language_state::{LanguageInput, LanguageLimits},
    };
    let pack = header();
    let packs = [pack.as_slice()];
    let program = source(
        0xabc0_0001,
        &[
            vec![0, 8, 1, 1, 1, 0x13, 9, 0, 9, 0, 0],
            vec![0, 8, 2, 1, 1, 0x15, 3, 0x15, 15],
        ],
        &[],
        1,
    )?;
    let (_, _, report) = run_with_context(
        &[program.input()],
        &[],
        super::ControlOptions::default(),
        RuntimeInputs {
            environment: None,
            language: Some(LanguageInput {
                packs: &packs,
                selected: 1,
                limits: LanguageLimits::default(),
            }),
        },
    )?;
    let report = report.ok_or("language report")?;
    assert!(
        report
            .events
            .iter()
            .filter(|event| event.stage < 5)
            .all(|event| event.files.iter().all(|file| file.maps.is_empty()))
    );
    let final_file = report
        .events
        .last()
        .and_then(|event| event.files.first())
        .ok_or("final file")?;
    assert_eq!(final_file.features, 1 << 8);
    let map = final_file.maps.get(&1).ok_or("map")?;
    assert_eq!(map.genders, [(9, 0), (9, 0)]);
    assert_eq!(map.plural, 3);
    Ok(())
}

#[path = "language_limits_tests.rs"]
mod limits;

#[test]
fn mapped_choices_charge_cumulative_intermediate_and_copied_bytes() -> Result {
    let map = LanguageMap {
        genders: vec![(1, 0)],
        ..LanguageMap::default()
    };
    let input = super::text_translate::Input {
        raw: &[
            0x9a, 0x13, 0x80, 0x9a, 0x10, 1, b'A', 0x9a, 0x11, b'D', 0x9a, 0x12,
        ],
        newlines: true,
        offset: 0,
        context: super::text_mapped::TextContext {
            map: Some(&map),
            genders: 2,
            cases: 0,
        },
    };
    for limit in [19, 20] {
        let result = super::text_translate::translate_in(
            input,
            &mut super::text::Budget::new(super::ScanLimits {
                translated_bytes: limit,
                ..super::ScanLimits::default()
            }),
            super::string_ids::original,
        );
        if limit == 19 {
            assert!(matches!(
                result,
                Err(super::ScanError::ResourceLimit {
                    resource: "translated bytes",
                    ..
                })
            ));
        } else {
            assert_eq!(result?, [0xee, 0x80, 0xbd, 0, 2, 1, 1, b'A', b'D']);
        }
    }
    Ok(())
}
