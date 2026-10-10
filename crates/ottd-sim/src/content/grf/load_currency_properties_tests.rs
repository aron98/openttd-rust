use super::{ControlOptions, Result, programs, run};

fn bytes(value: &impl AsRef<[u8]>) -> &[u8] {
    value.as_ref()
}

fn property(kind: u8, first: u16, count: u8, raw: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, 8, 1, count, 255];
    bytes.extend(first.to_le_bytes());
    bytes.push(kind);
    bytes.extend(raw);
    bytes
}

#[test]
fn rate_narrows_after_division_when_multiplier_exceeds_u16() -> Result {
    let source = programs::source(
        7,
        &[property(0x0b, 0, 1, &65_536_000_u32.to_le_bytes())],
        &[],
        1,
    )?;
    let (_, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        report
            .currency
            .ok_or("currency")?
            .owners
            .entries
            .first()
            .ok_or("owner")?
            .rate,
        0
    );
    Ok(())
}

#[test]
fn options_drop_invalid_separator_and_ignore_high_bits() -> Result {
    let source = programs::source(7, &[property(0x0c, 31, 1, &[0x80, 0xff])], &[], 1)?;
    let (_, report) = run(&[source], None, ControlOptions::default())?;
    let currency = report.currency.ok_or("currency")?;
    let owner = currency.owners.entries.get(31).ok_or("custom")?;
    assert_eq!((&owner.separator, owner.symbol_pos), (&String::new(), 1));
    Ok(())
}

#[test]
fn prefix_keeps_surrogate_bytes_when_native_decoder_accepts_them() -> Result {
    let source = programs::source(7, &[property(0x0d, 0, 1, &[0xed, 0xa0, 0x80, 0])], &[], 1)?;
    let (_, report) = run(&[source], None, ControlOptions::default())?;
    let currency = report.currency.ok_or("currency")?;
    assert_eq!(
        bytes(&currency.owners.entries.first().ok_or("owner")?.prefix),
        &[0xed, 0xa0, 0x80]
    );
    Ok(())
}

#[test]
fn suffix_skips_bad_utf8_and_replaces_control_when_read_as_four_bytes() -> Result {
    let source = programs::source(7, &[property(0x0e, 0, 1, &[0xff, b'A', 1, 0])], &[], 2)?;
    let (_, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        bytes(
            &report
                .currency
                .ok_or("currency")?
                .owners
                .entries
                .first()
                .ok_or("owner")?
                .suffix
        ),
        b"A?"
    );
    Ok(())
}

#[test]
fn euro_date_is_not_clamped_when_below_custom_ui_minimum() -> Result {
    let source = programs::source(7, &[property(0x0f, 31, 1, &2_u16.to_le_bytes())], &[], 1)?;
    let (_, report) = run(&[source], None, ControlOptions::default())?;
    assert_eq!(
        report
            .currency
            .ok_or("currency")?
            .owners
            .entries
            .get(31)
            .ok_or("custom")?
            .to_euro,
        2
    );
    Ok(())
}

#[test]
fn malformed_reserve_disables_before_any_multiplier_write() -> Result {
    let source = programs::source(7, &[property(0x0b, 0, 2, &[0xe8, 3, 0, 0, 1])], &[], 1)?;
    let (report, language) = run(&[source], None, ControlOptions::default())?;
    let file = report.files.first().ok_or("file")?;
    assert_eq!(file.status, super::LoadStatus::Disabled);
    assert!(matches!(
        file.errors.first().ok_or("error")?.failure,
        super::super::LoadFailure::ReadBounds
    ));
    assert!(language.currency.is_none());
    Ok(())
}
