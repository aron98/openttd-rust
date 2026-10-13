//! Ordered Action14 metadata and explicit native parameter operations.
use ottd_sim::content::grf::{
    Palette, ScanError, ScanLimits, ScanOptions, ScanStatus, scan_file, scan_file_with_options,
    translate_fresh_text,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn binary(id: [u8; 4], value: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![b'B'];
    out.extend(id);
    out.extend(u16::try_from(value.len())?.to_le_bytes());
    out.extend(value);
    Ok(out)
}

fn branch(id: [u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![b'C'];
    out.extend(id);
    for child in children {
        out.extend(child);
    }
    out.push(0);
    out
}

fn file(nodes: &[Vec<u8>]) -> Result<Vec<u8>> {
    let mut action = vec![0x14];
    action.extend(branch(*b"INFO", nodes));
    action.push(0);
    let mut out = vec![4, 0, 255, 0, 0, 0, 0];
    for payload in [action.as_slice(), b"\x08\x08TESTname\0"] {
        out.extend(u16::try_from(payload.len())?.to_le_bytes());
        out.push(255);
        out.extend(payload);
    }
    out.extend([0, 0]);
    Ok(out)
}

fn text(id: [u8; 4], language: u8, raw: &[u8]) -> Vec<u8> {
    let mut out = vec![b'T'];
    out.extend(id);
    out.push(language);
    out.extend(raw);
    out.push(0);
    out
}

#[test]
fn native_text_choices_unicode_and_language_replacement() -> Result {
    let bytes = file(&[
        text(
            *b"DESC",
            2,
            b"prefix\x9a\x14\x9a\x11case\x9a\x15\x80\x9a\x11default\x9a\x12end\x9a\x12suffix",
        ),
        text(*b"DESC", 0, b"American"),
        text(*b"DESC", 1, b"English"),
        text(*b"DESC", 1, b"\xc3\x9e\xed\xa0\x80\xee\x80\x8dX"),
    ])?;
    let result = scan_file(&bytes)?;
    let list = &result.static_info.description;
    assert_eq!(
        list.entries()
            .iter()
            .map(|v| v.language)
            .collect::<Vec<_>>(),
        vec![2, 0, 1]
    );
    assert_eq!(
        list.select(2).ok_or("choice")?.translated,
        b"prefixcasedefaultendsuffix"
    );
    assert_eq!(
        list.select(1).ok_or("unicode")?.translated,
        b"\xed\xa0\x80\nX"
    );
    assert_eq!(list.select(255).ok_or("fallback")?.translated, b"American");
    Ok(())
}

#[test]
fn ordered_versions_and_palette_are_finalized() -> Result {
    let bytes = file(&[
        binary(*b"MINV", &9_u32.to_le_bytes())?,
        binary(*b"VRSN", &7_u32.to_le_bytes())?,
        binary(*b"MINV", &3_u32.to_le_bytes())?,
        binary(*b"PALS", b"D")?,
        binary(*b"BLTR", b"3")?,
    ])?;
    let result = scan_file_with_options(
        &bytes,
        ScanOptions {
            default_palette: Palette::Windows,
            ..ScanOptions::default()
        },
    )?;
    assert_eq!(result.static_info.version, 7);
    assert_eq!(result.static_info.min_loadable_version, 3);
    for old in [0, 2, 3, 4, 6, 7, 8, u32::MAX] {
        assert_eq!(
            result.static_info.is_compatible(old),
            (3..=7).contains(&old)
        );
    }
    assert_eq!(result.static_info.palette_bits, 0x14);
    assert_eq!(
        result.static_info.name.select(1).ok_or("name")?.translated,
        b"name"
    );
    Ok(())
}

#[test]
fn overlapping_defaults_follow_metadata_order_and_wrong_length_sets_flag() -> Result {
    let bytes = file(&[
        branch(
            *b"PARA",
            &[
                branch(
                    0_u32.to_le_bytes(),
                    &[
                        binary(*b"MASK", &[2, 0, 4])?,
                        binary(*b"DFLT", &[15, 0, 0, 0])?,
                    ],
                ),
                branch(
                    3_u32.to_le_bytes(),
                    &[binary(*b"MASK", &[2, 2, 2])?, binary(*b"DFLT", &[])?],
                ),
            ],
        ),
        binary(*b"NPAR", &[0])?,
    ])?;
    let result = scan_file(&bytes)?;
    assert_eq!(result.static_info.num_valid_params, 0);
    assert_eq!(result.static_info.parameters.len(), 4);
    assert!(
        result
            .static_info
            .parameters
            .get(1)
            .ok_or("hole")?
            .is_none()
    );
    assert_eq!(result.static_info.default_parameters(), vec![0, 0, 3]);
    Ok(())
}

#[test]
fn host_bounds_are_cumulative_and_distinct_from_native_disabling() -> Result {
    let bytes = file(&[text(*b"DESC", 2, b"abc"), text(*b"DESC", 2, b"def")])?;
    let defaults = ScanLimits::default();
    for (resource, limits) in [
        (
            "bytes",
            ScanLimits {
                bytes: bytes.len().saturating_sub(1),
                ..defaults
            },
        ),
        (
            "records",
            ScanLimits {
                records: 1,
                ..defaults
            },
        ),
        (
            "nodes",
            ScanLimits {
                nodes: 2,
                ..defaults
            },
        ),
        (
            "nesting",
            ScanLimits {
                nesting: 0,
                ..defaults
            },
        ),
        (
            "translated bytes",
            ScanLimits {
                translated_bytes: 5,
                ..defaults
            },
        ),
    ] {
        assert!(
            matches!(scan_file_with_options(&bytes, ScanOptions {limits,..ScanOptions::default()}), Err(ScanError::ResourceLimit {resource: actual,..}) if actual == resource)
        );
    }
    let exact = ScanLimits {
        bytes: bytes.len(),
        records: 2,
        nodes: 3,
        nesting: 1,
        translated_bytes: 10,
    };
    assert!(
        scan_file_with_options(
            &bytes,
            ScanOptions {
                limits: exact,
                ..ScanOptions::default()
            }
        )?
        .accepted
    );
    assert!(translate_fresh_text(b"abc", false, 3).is_ok());
    assert!(matches!(
        translate_fresh_text(b"abc", false, 2),
        Err(ScanError::ResourceLimit { .. })
    ));
    assert!(matches!(
        translate_fresh_text(b"\x9a\x14\x9a\x10\x01discarded", false, 8),
        Err(ScanError::ResourceLimit { .. })
    ));
    let nested = file(&[branch(*b"WHAT", &[branch(*b"WHAT", &[])])])?;
    assert!(matches!(
        scan_file_with_options(
            &nested,
            ScanOptions {
                limits: ScanLimits {
                    nesting: 2,
                    ..defaults
                },
                ..ScanOptions::default()
            }
        ),
        Err(ScanError::ResourceLimit {
            resource: "nesting",
            ..
        })
    ));
    Ok(())
}

#[test]
fn false_return_and_read_bounds_retain_applied_metadata() -> Result {
    for (tag, status) in [
        (b"NPAR", ScanStatus::Unknown),
        (b"WHAT", ScanStatus::Disabled),
    ] {
        let mut overlong = vec![b'B'];
        overlong.extend(tag);
        overlong.extend([255, 255]);
        let bytes = file(&[binary(*b"VRSN", &7_u32.to_le_bytes())?, overlong])?;
        let result = scan_file(&bytes)?;
        assert_eq!(result.static_info.version, 7);
        assert_eq!(result.status, status);
        assert_eq!(result.accepted, status == ScanStatus::Unknown);
    }
    Ok(())
}

#[test]
fn native_bit_setter_clears_window_but_does_not_mask_inserted_value() -> Result {
    for width in [0, 1] {
        let bytes = file(&[branch(
            *b"PARA",
            &[branch(
                0_u32.to_le_bytes(),
                &[binary(*b"MASK", &[0, 0, width])?],
            )],
        )])?;
        let result = scan_file(&bytes)?;
        let parameter = result
            .static_info
            .parameters
            .first()
            .and_then(Option::as_ref)
            .ok_or("parameter")?;
        let mut values = vec![0];
        parameter.write(&mut values, 7);
        assert_eq!(values, vec![7]);
        assert_eq!(parameter.read(&values), u32::from(width != 0));
    }
    Ok(())
}

#[test]
fn definition_context_is_retained_and_requested_language_selects_name() -> Result {
    let bytes = file(&[text(*b"NAME", 2, b"localized")])?;
    let result = scan_file_with_options(
        &bytes,
        ScanOptions {
            language: 2,
            ..ScanOptions::default()
        },
    )?;
    let selected = result.selected_name().ok_or("selected name")?;
    assert_eq!(selected.raw, b"localized");
    assert_eq!(selected.definition_grfid, 0);
    let action8 = result.static_info.name.select(127).ok_or("action8 name")?;
    assert_eq!(action8.raw, b"name");
    assert_eq!(action8.definition_grfid, u32::from_le_bytes(*b"TEST"));
    Ok(())
}
