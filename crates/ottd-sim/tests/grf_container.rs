//! Actual `NewGRF` framing, identity and file-scan semantics.
use ottd_sim::content::grf::{
    GrfContainer, GrfParseError, ParseLimits, RecordKind, ScanError, ScanStatus, scan_file,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn pseudo(out: &mut Vec<u8>, data: &[u8], version: u8) -> Result {
    if version == 1 {
        out.extend(u16::try_from(data.len())?.to_le_bytes());
    } else {
        out.extend(u32::try_from(data.len())?.to_le_bytes());
    }
    out.push(0xff);
    out.extend(data);
    Ok(())
}

fn file(actions: &[&[u8]], version: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    if version == 2 {
        out.extend(b"\0\0GRF\x82\r\n\x1a\n");
        out.extend([0; 4]);
        out.push(0);
    }
    pseudo(&mut out, &[0; 4], version)?;
    for action in actions {
        pseudo(&mut out, action, version)?;
    }
    out.extend(vec![0; if version == 1 { 2 } else { 4 }]);
    if version == 2 {
        let offset = u32::try_from(out.len().checked_sub(14).ok_or("offset")?)?;
        out.get_mut(10..14)
            .ok_or("header")?
            .copy_from_slice(&offset.to_le_bytes());
        out.extend([0; 4]);
    }
    Ok(out)
}

#[test]
fn unchanged_contract_and_first_action8_scan() -> Result {
    let bytes = include_bytes!("../../../fixtures/content/contract-speed.grf");
    let parsed = GrfContainer::parse(bytes)?;
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.records.len(), 3);
    let scan = scan_file(bytes)?;
    assert_eq!(scan.status, ScanStatus::Unknown);
    let metadata = scan.metadata.ok_or("metadata")?;
    assert_eq!(metadata.name, b"Contract Speed");
    assert_eq!(
        scan.identity.ok_or("identity")?.grfid,
        u32::from_le_bytes(*b"RUST")
    );
    let mut trailing = file(&[b"\x08\x08TESTfirst\0", b"\x08\x08FAILsecond\0"], 1)?;
    trailing.extend([255, 255, 255]);
    assert_eq!(
        scan_file(&trailing)?.metadata.ok_or("first")?.name,
        b"first"
    );
    Ok(())
}

#[test]
fn v2_identity_excludes_sprite_data_and_retains_variants() -> Result {
    let mut bytes = file(&[b"\x08\x08TESTraw\xff\0"], 2)?;
    bytes.truncate(bytes.len().saturating_sub(4));
    for data in [b"\x04\x00a", b"\x04\x02b"] {
        bytes.extend(7_u32.to_le_bytes());
        bytes.extend(3_u32.to_le_bytes());
        bytes.extend(data);
    }
    bytes.extend([0; 4]);
    let parsed = GrfContainer::parse(&bytes)?;
    assert_eq!(parsed.sprites.len(), 2);
    let identity = scan_file(&bytes)?.identity.ok_or("identity")?;
    *bytes.last_mut().ok_or("last")? = 2;
    assert_eq!(scan_file(&bytes)?.identity.ok_or("identity")?, identity);
    assert!(GrfContainer::parse(&bytes).is_err());
    Ok(())
}

#[test]
fn statuses_metadata_and_unsupported_are_distinct() -> Result {
    let invalid = file(&[b"\x08\x01TESTname"], 1)?;
    let scan = scan_file(&invalid)?;
    assert!(scan.invalid_version);
    assert_eq!(scan.metadata.ok_or("metadata")?.name, b"name");
    let system_bytes = file(&[b"\x08\x08\xffABCname\0"], 1)?;
    let system = scan_file(&system_bytes)?;
    assert!(system.system);
    assert!(!system.accepted);
    assert!(matches!(
        scan_file(&file(&[b"\x14\0"], 1)?),
        Err(ScanError::UnsupportedAction(0x14))
    ));
    let truncated_bytes = file(&[b"\x08\x08"], 1)?;
    let truncated = scan_file(&truncated_bytes)?;
    assert_eq!(truncated.status, ScanStatus::Disabled);
    assert!(truncated.identity.is_none());
    Ok(())
}

#[test]
fn compressed_inline_sprite_uses_physical_not_declared_length() -> Result {
    let mut bytes = Vec::new();
    pseudo(&mut bytes, &[0; 4], 1)?;
    pseudo(&mut bytes, &[1, 0, 1, 1], 1)?;
    bytes.extend(12_u16.to_le_bytes());
    bytes.push(0);
    bytes.extend([0; 7]);
    bytes.extend([1, 42, 0xe8, 1]); // one literal + three-byte backreference = four decoded bytes
    pseudo(&mut bytes, b"\x08\x08TESTafter\0", 1)?;
    bytes.extend([0; 2]);
    let parsed = GrfContainer::parse(&bytes)?;
    assert!(matches!(
        parsed.records.get(1).ok_or("record")?.kind,
        RecordKind::InlineSprite { .. }
    ));
    assert_eq!(
        scan_file(&bytes)?.metadata.ok_or("metadata")?.name,
        b"after"
    );
    for end in 0..bytes.len() {
        assert!(GrfContainer::parse(bytes.get(..end).ok_or("prefix")?).is_err());
    }
    Ok(())
}

#[test]
fn section_offsets_compression_and_limits_are_checked() -> Result {
    let valid = file(&[b"\x08\x08TESTname\0"], 2)?;
    for offset in [0, 1, 1 << 30, u32::MAX] {
        let mut bytes = valid.clone();
        bytes
            .get_mut(10..14)
            .ok_or("offset")?
            .copy_from_slice(&offset.to_le_bytes());
        assert!(GrfContainer::parse(&bytes).is_err());
    }
    let mut compressed = valid.clone();
    *compressed.get_mut(14).ok_or("compression")? = 1;
    assert!(matches!(
        GrfContainer::parse(&compressed),
        Err(GrfParseError::Compression(1))
    ));
    let mut signature = valid.clone();
    *signature.get_mut(3).ok_or("signature")? = 0;
    assert!(matches!(
        GrfContainer::parse(&signature),
        Err(GrfParseError::Signature)
    ));
    for end in 0..valid.len() {
        assert!(GrfContainer::parse(valid.get(..end).ok_or("prefix")?).is_err());
    }
    for limits in [
        ParseLimits {
            bytes: 1,
            records: 100,
        },
        ParseLimits {
            bytes: 1000,
            records: 0,
        },
    ] {
        assert_eq!(
            GrfContainer::parse_with_limits(&valid, limits).err(),
            Some(GrfParseError::ResourceLimit)
        );
    }
    Ok(())
}

#[test]
fn file_scan_skips_real_and_pseudo_records_by_native_counts() -> Result {
    for action in [
        &[1, 0, 1, 1][..],
        &[5, 0, 1],
        &[0x0a, 1, 1, 0, 0],
        &[0x11, 1, 0],
        &[0x12, 1, 0, 1, 0, 0],
    ] {
        let bytes = file(
            &[action, b"\x08\x08FAILskipped\0", b"\x08\x08TESTname\0"],
            1,
        )?;
        assert_eq!(
            scan_file(&bytes)?.metadata.ok_or("scan")?.grfid,
            u32::from_le_bytes(*b"TEST")
        );
    }
    let extended = file(
        &[
            &[1, 0, 0, 255, 0, 0, 255, 1, 0, 255, 1, 0],
            b"\x14\0",
            b"\x08\x08TESTname\0",
        ],
        1,
    )?;
    assert!(scan_file(&extended)?.accepted);
    let no_identity = file(&[b"\x08\x08\0\0\0\0name\0"], 1)?;
    assert!(scan_file(&no_identity)?.identity.is_none());
    let unknown = file(&[b"\xa0", b"\x08\x08TESTname\0"], 1)?;
    assert!(scan_file(&unknown)?.accepted);
    Ok(())
}
