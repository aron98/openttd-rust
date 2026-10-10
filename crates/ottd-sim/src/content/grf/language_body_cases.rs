use super::{
    Pack, Result,
    cases::{Case, base},
};

pub(super) fn last_record(bytes: &[u8]) -> Result<usize> {
    let pack = Pack::header(bytes)?;
    let count = pack.tables.iter().fold(0_usize, |total, value| {
        total.saturating_add(usize::from(*value))
    });
    let mut position = 572_usize;
    let mut last = position;
    for _ in 0..count {
        last = position;
        let first = *bytes.get(position).ok_or("string length")?;
        position = position.checked_add(1).ok_or("string offset")?;
        let length = if first >= 0xc0 {
            let low = *bytes.get(position).ok_or("extended length")?;
            position = position.checked_add(1).ok_or("string offset")?;
            (usize::from(first & 63) << 8) | usize::from(low)
        } else {
            usize::from(first)
        };
        position = position.checked_add(length).ok_or("string offset")?;
        if position > bytes.len() {
            return Err("source pack body".into());
        }
    }
    Ok(last)
}

pub(super) fn all(bytes: &[u8]) -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    let first = *bytes.get(572).ok_or("first string length")?;
    let (length, prefix) = if first >= 0xc0 {
        (
            (usize::from(first & 63) << 8)
                | usize::from(*bytes.get(573).ok_or("first extended length")?),
            2_usize,
        )
    } else {
        (usize::from(first), 1_usize)
    };
    let tail = bytes
        .get(572_usize.saturating_add(prefix).saturating_add(length)..)
        .ok_or("first string tail")?;
    for length in [0_usize, 1, 191, 192, 193, 255, 256, 16383] {
        let mut replacement = bytes.get(..572).ok_or("header")?.to_vec();
        if length < 192 {
            replacement.push(u8::try_from(length)?);
        } else {
            replacement.extend([
                0xc0 | u8::try_from(length >> 8)?,
                u8::try_from(length & 255)?,
            ]);
        }
        replacement.extend(std::iter::repeat_n(b'X', length));
        replacement.extend_from_slice(tail);
        let mut case = base(format!("body-first-length-{length}"), bytes, &[])?;
        case.packs = vec![replacement];
        cases.push(case);
    }
    let mut padded = base("body-exact-read-cap".into(), bytes, &[])?;
    padded.packs.first_mut().ok_or("pack")?.resize(1 << 20, 0);
    cases.push(padded);
    let mut without_tail = base("body-without-writer-final-zero".into(), bytes, &[])?;
    let pack = without_tail.packs.first_mut().ok_or("pack")?;
    if pack.pop() != Some(0) {
        return Err("original writer terminator".into());
    }
    cases.push(without_tail);
    Ok(cases)
}
