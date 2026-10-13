//! Native lazy sprite skipping remains distinct from strict container parsing.
use ottd_sim::content::grf::{GrfContainer, scan_file};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn source(length: u32, flags: u8, pixels: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = b"\0\0GRF\x82\r\n\x1a\n\0\0\0\0\0".to_vec();
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend([255, 0, 0, 0, 0]);
    bytes.extend(3_u32.to_le_bytes());
    bytes.extend([255, 5, 0, 1]);
    bytes.extend(length.to_le_bytes());
    bytes.push(flags);
    bytes.extend([0; 7]);
    bytes.extend(pixels);
    bytes.extend(7_u32.to_le_bytes());
    bytes.extend([255, 8, 8, 4, 3, 2, 1, 0]);
    bytes.extend([0; 4]);
    let offset = u32::try_from(bytes.len().checked_sub(14).ok_or("header")?)?;
    bytes
        .get_mut(10..14)
        .ok_or("offset field")?
        .copy_from_slice(&offset.to_le_bytes());
    bytes.extend([0; 4]);
    Ok(bytes)
}

#[test]
fn filescan_observes_action8_after_native_literal_overrun_stop() -> Result {
    let bytes = source(9, 0, &[2])?;
    let report = scan_file(&bytes).expect("original scanner reaches Action8");
    assert_eq!(
        report.identity.expect("accepted identity").grfid,
        0x0102_0304
    );
    assert!(GrfContainer::parse(&bytes).is_err());
    Ok(())
}

#[test]
fn filescan_truncates_skipped_inline_length_like_native_uint16() -> Result {
    let bytes = source(65545, 2, &[42])?;
    let report = scan_file(&bytes).expect("original scanner reaches Action8");
    assert_eq!(
        report.identity.expect("accepted identity").grfid,
        0x0102_0304
    );
    assert!(GrfContainer::parse(&bytes).is_err());
    Ok(())
}
