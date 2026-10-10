use super::super::{
    load_currency::{CurrencyOwners, CurrencyState},
    load_currency_native::legacy_owner,
    load_currency_properties::Property,
    records::Reader,
};
use super::{ControlLoadError, ControlOptions, Result, programs, run};

#[test]
fn symbol_bounds_preserve_prior_value_with_stage_specific_consumption() -> Result {
    for activation in [false, true] {
        let mut currency = CurrencyState::default();
        let initial = currency.owners.clone();
        let mut reader = Reader {
            bytes: &[b'A', 0, 0, 0, b'B', b'C'],
            pos: 0,
        };
        if let Some(value) = Property::Prefix.read(&mut reader, activation)? {
            value.apply(&mut currency, 0);
        }
        assert!(Property::Prefix.read(&mut reader, activation).is_err());
        assert_eq!(reader.remaining(), if activation { 0 } else { 2 });
        if activation {
            assert_eq!(currency.owners.entries.first().ok_or("owner")?.prefix, b"A");
        } else {
            assert_eq!(currency.owners, initial);
        }
    }
    Ok(())
}

#[test]
fn all_single_byte_options_follow_original_printability() -> Result {
    for byte in 0..=255_u8 {
        let mut currency = CurrencyState::default();
        let raw = [byte, 0xff];
        let mut reader = Reader {
            bytes: &raw,
            pos: 0,
        };
        Property::Options
            .read(&mut reader, true)?
            .ok_or("activation value")?
            .apply(&mut currency, 31);
        let owner = currency.owners.entries.get(31).ok_or("custom")?;
        let expected = match byte {
            0 | 128..=255 => Vec::new(),
            1..=31 => vec![b'?'],
            byte => vec![byte],
        };
        assert_eq!(owner.separator.as_bytes(), expected);
        assert_eq!(owner.symbol_pos, 1);
    }
    Ok(())
}

#[test]
fn currency_symbol_validation_preserves_defined_native_bytes() -> Result {
    let cases: &[([u8; 4], &[u8])] = &[
        ([0xed, 0xa0, 0x80, 0], &[0xed, 0xa0, 0x80]),
        ([0xed, 0xbf, 0xbf, 0], &[0xed, 0xbf, 0xbf]),
        ([0xf4, 0x8f, 0xbf, 0xbf], &[0xf4, 0x8f, 0xbf, 0xbf]),
        ([0xff, b'A', 1, 0], b"A?"),
        ([b'A', 0, b'B', 1], b"A"),
        ([0xc0, 0x80, b'Z', 0], b"Z"),
        ([0xee, 0x80, 0x80, 0], b"?"),
        ([0xee, 0x8b, 0xbf, 0], b"?"),
        ([0xee, 0x8c, 0x80, 0], &[0xee, 0x8c, 0x80]),
        ([0x7f, 0xc2, 0x80, 0], &[0x7f, 0xc2, 0x80]),
    ];
    for &(raw, expected) in cases {
        let mut currency = CurrencyState::default();
        let mut reader = Reader {
            bytes: &raw,
            pos: 0,
        };
        Property::Suffix
            .read(&mut reader, true)?
            .ok_or("activation value")?
            .apply(&mut currency, 0);
        assert_eq!(
            currency.owners.entries.first().ok_or("owner")?.suffix,
            expected
        );
    }
    Ok(())
}

#[test]
fn property_destination_wraps_and_custom_reset_preserves_symbol_bytes() -> Result {
    let mut currency = CurrencyState::default();
    for (property, raw, index) in [
        (Property::Rate, [0xd0, 7, 0, 0], 65536),
        (Property::Prefix, [0xed, 0xa0, 0x80, 0], 31),
        (Property::Suffix, [b'X', 0, 0, 0], 255),
    ] {
        let mut reader = Reader {
            bytes: &raw,
            pos: 0,
        };
        property
            .read(&mut reader, true)?
            .ok_or("activation value")?
            .apply(&mut currency, index);
    }
    assert_eq!(currency.owners.entries.first().ok_or("owner")?.rate, 2);
    let custom = currency.owners.entries.get(31).ok_or("custom")?;
    assert_eq!(
        CurrencyOwners::reset(Some(custom)).entries.get(31),
        Some(custom)
    );
    assert_eq!(custom.prefix, [0xed, 0xa0, 0x80]);
    assert_eq!(currency.owners.entries.first().ok_or("owner")?.suffix, b"");
    Ok(())
}

#[test]
fn legacy_projection_refuses_surrogate_bytes_without_replacement() -> Result {
    let mut owner = CurrencyOwners::default()
        .entries
        .first()
        .ok_or("owner")?
        .clone();
    legacy_owner(&owner)?;
    owner.prefix = vec![0xed, 0xa0, 0x80];
    assert!(legacy_owner(&owner).is_err());
    Ok(())
}

#[test]
fn repeated_value_writes_obey_cumulative_trace_budget() -> Result {
    let value = vec![0, 8, 1, 1, 0, 0x0d, b'A', b'B', b'C', b'D'];
    let options = ControlOptions {
        max_trace_bytes: 128 * 1024,
        ..ControlOptions::default()
    };
    let single = programs::source(7, std::slice::from_ref(&value), &[], 1)?;
    run(&[single], None, options)?;
    let repeated = programs::source(7, &vec![value; 100], &[], 1)?;
    let error = run(&[repeated], None, options)
        .expect_err("cumulative owner snapshots must exhaust budget");
    assert!(matches!(
        error.downcast_ref::<ControlLoadError>(),
        Some(ControlLoadError::ResourceLimit { .. })
    ));
    Ok(())
}
