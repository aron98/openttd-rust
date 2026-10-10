use super::{
    load::{RuntimeInputs, run_with_context},
    load_context_tests::grf_control_cases as programs,
    load_language_state::{LanguageInput, LanguageLimits, LanguageReport},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run(sources: &[programs::Source]) -> Result<(super::ControlLoadReport, LanguageReport)> {
    let mut pack = vec![0; 572];
    pack.get_mut(..4)
        .ok_or("magic")?
        .copy_from_slice(&0x474e_414c_u32.to_le_bytes());
    pack.get_mut(4..8)
        .ok_or("version")?
        .copy_from_slice(&0x2ad1_09ab_u32.to_le_bytes());
    let packs = [pack.as_slice()];
    let inputs = sources
        .iter()
        .map(programs::Source::input)
        .collect::<Vec<_>>();
    let (report, _, language) = run_with_context(
        &inputs,
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
    )?;
    Ok((report, language.ok_or("language report")?))
}

#[test]
fn absent_translation_target_does_not_read_missing_body() -> Result {
    let source = programs::source(7, &[vec![0x13, 9, 0, 0, 0]], &[], 1)?;
    let (report, language) = run(&[source])?;
    assert_eq!(
        report.files.first().ok_or("file")?.status,
        super::LoadStatus::Activated
    );
    assert!(language.strings.is_empty());
    Ok(())
}

#[test]
fn earlier_target_receives_later_translation() -> Result {
    let target = programs::source(7, &[vec![4, 0, 0x81, 1, 0, 0xd8, b'A', 0]], &[], 1)?;
    let translator =
        programs::source(8, &[vec![0x13, 7, 0, 0, 0, 1, 1, 0, 0xd8, b'B', 0]], &[], 1)?;
    let (_, language) = run(&[target, translator])?;
    assert_eq!(language.strings.len(), 1);
    let entry = language.strings.first().ok_or("entry")?;
    assert_eq!(entry.key.grfid, 7);
    assert_eq!(entry.translations, [(1, b"B".to_vec())]);
    Ok(())
}

#[test]
fn later_target_disables_translator_before_body_read() -> Result {
    let translator = programs::source(
        8,
        &[
            vec![0x10, 42],
            vec![4, 0, 0x81, 1, 0, 0xd8, b'R', 0],
            vec![0x13, 7, 0, 0, 0],
        ],
        &[],
        1,
    )?;
    let target = programs::source(7, &[], &[], 1)?;
    let (report, language) = run(&[translator, target])?;
    let first = report.files.first().ok_or("translator")?;
    assert_eq!(first.status, super::LoadStatus::Disabled);
    assert_eq!(first.errors.len(), 1);
    assert_eq!(
        format!("{:?}", first.errors.first().ok_or("error")?.failure),
        "LoadAfter"
    );
    let failure = language.translation_errors.first().ok_or("full error")?;
    assert_eq!(failure.file, 0);
    assert_eq!(failure.line, 4);
    assert_eq!(failure.message, 0xb29);
    assert_eq!(failure.severity, 0xb21);
    assert_eq!(failure.parameters, [4, 0]);
    assert_eq!(failure.data, b"(undefined string)");
    assert!(failure.custom_message.is_empty());
    assert_eq!(language.strings.len(), 1);
    assert_eq!(
        language
            .strings
            .first()
            .ok_or("retained string")?
            .translations,
        [(1, b"R".to_vec())]
    );
    Ok(())
}

#[test]
fn unimplemented_string_owners_remain_explicitly_unsupported() -> Result {
    for record in [
        vec![4, 0, 1, 1, 0, b'E', 0],
        vec![4, 21, 1, 1, 0xff, 0, 1, b'B', 0],
        vec![4, 4, 0x81, 1, 0, 0xc4, b'S', 0],
    ] {
        let source = programs::source(7, &[record], &[], 1)?;
        let error = run(&[source]).err().ok_or("owner was admitted")?;
        assert!(matches!(
            error.downcast_ref::<super::ControlLoadError>(),
            Some(super::ControlLoadError::Unsupported { .. })
        ));
    }
    Ok(())
}

#[test]
fn language_free_control_executor_does_not_admit_custom_strings() -> Result {
    let source = programs::source(7, &[vec![4, 0, 0x81, 1, 0, 0xd8, b'A', 0]], &[], 1)?;
    let result = run_with_context(
        &[source.input()],
        &[],
        super::ControlOptions::default(),
        RuntimeInputs {
            environment: None,
            language: None,
        },
    );
    assert!(matches!(
        result,
        Err(super::ControlLoadError::Unsupported { .. })
    ));
    Ok(())
}
