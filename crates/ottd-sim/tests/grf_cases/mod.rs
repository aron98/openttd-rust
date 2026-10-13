//! Native test byte streams independent of the production parser.
use super::{Result, action8, encode};

type Case = (&'static str, Vec<u8>, bool);
/// Build sprite and lazy-scan cases.
/// # Errors
/// Returns an error if generated lengths cannot fit their wire fields.
pub fn additional() -> Result<Vec<Case>> {
    let info = action8(8, *b"REAL", "sprites");
    let mut cases = Vec::new();
    for (name, flags, decoded, physical) in [
        ("v1-raw", 2, 4_u16, vec![1, 2, 3, 4]),
        ("v1-compressed", 0, 4, vec![1, 42, 0xe8, 1]),
        ("v1-zero-literal", 0, 128, [vec![0], vec![12; 128]].concat()),
    ] {
        let mut bytes = vec![4, 0, 255, 0, 0, 0, 0, 4, 0, 255, 1, 0, 1, 1];
        bytes.extend(decoded.checked_add(8).ok_or("length")?.to_le_bytes());
        bytes.push(flags);
        bytes.extend([0; 7]);
        bytes.extend(physical);
        bytes.extend(u16::try_from(info.len())?.to_le_bytes());
        bytes.push(255);
        bytes.extend(&info);
        bytes.extend([0; 2]);
        cases.push((name, bytes, true));
    }
    let mut bytes = encode(&[vec![1, 0, 1, 1]], 2)?;
    bytes.truncate(bytes.len().saturating_sub(8));
    bytes.extend(4_u32.to_le_bytes());
    bytes.push(253);
    bytes.extend(7_u32.to_le_bytes());
    bytes.extend(u32::try_from(info.len())?.to_le_bytes());
    bytes.push(255);
    bytes.extend(&info);
    bytes.extend([0; 4]);
    let offset = u32::try_from(bytes.len().checked_sub(14).ok_or("offset")?)?;
    bytes
        .get_mut(10..14)
        .ok_or("offset")?
        .copy_from_slice(&offset.to_le_bytes());
    for (id, data) in [
        (7_u32, b"\x04\x00a"),
        (7, b"\x04\x02b"),
        (9, b"\x04\x00c"),
        (7, b"\x04\x00d"),
    ] {
        bytes.extend(id.to_le_bytes());
        bytes.extend(3_u32.to_le_bytes());
        bytes.extend(data);
    }
    bytes.extend([0; 4]);
    cases.push(("v2-variants", bytes.clone(), true));
    let last_pixel = bytes.len().checked_sub(5).ok_or("pixel")?;
    *bytes.get_mut(last_pixel).ok_or("pixel")? = b'z';
    cases.push(("v2-pixel-mutation", bytes, true));
    let mut trailing = encode(std::slice::from_ref(&info), 1)?;
    trailing.truncate(trailing.len().saturating_sub(2));
    trailing.extend([255, 255, 255]);
    cases.push(("early-stop-before-malformed", trailing, false));
    let mut huge_offset = encode(&[info], 2)?;
    huge_offset
        .get_mut(10..14)
        .ok_or("offset")?
        .copy_from_slice(&(1_u32 << 30).to_le_bytes());
    cases.push(("native-md5-whole-file-offset", huge_offset, false));
    let mut oversized = encode(&[], 2)?;
    oversized.truncate(24);
    oversized.extend(1_048_577_u32.to_le_bytes());
    oversized.push(255);
    cases.push(("oversized-executed-pseudo", oversized, false));
    let unexpected = vec![4, 0, 255, 0, 0, 0, 0, 255, 255, 0];
    cases.push(("unexpected-real-before-payload", unexpected, false));
    for (name, parameter, before) in [
        ("parameter-zero-before", 0_u32, true),
        ("parameter-max-before", u32::MAX, true),
        ("parameter-max-after", u32::MAX, false),
    ] {
        let mut assignment = vec![0x0d, 0, 0, 255, 0];
        assignment.extend(parameter.to_le_bytes());
        let information = action8(8, *b"PARM", "parameter-order");
        let actions = if before {
            vec![assignment, information]
        } else {
            vec![information, assignment]
        };
        cases.push((name, encode(&actions, 1)?, true));
    }
    for (name, information) in [
        ("omitted-info", b"\x08\x08TESTname".as_slice()),
        ("empty-info", b"\x08\x08TESTname\0\0".as_slice()),
        ("empty-name", b"\x08\x08TEST\0description\0".as_slice()),
    ] {
        cases.push((name, encode(&[information.to_vec()], 1)?, true));
    }
    Ok(cases)
}
