use super::{Case, Result, set, single};
pub(super) fn cases() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for operation in 0..=12 {
        for (name, left, right) in [
            ("ordinary", 19, 3),
            ("zero", 0xffff_fff7, 0),
            ("negative", 0xffff_fff7, 0xffff_fffd),
            ("boundary", 0x8000_0000, 31),
        ] {
            cases.push(single(
                &format!("op-{operation:02x}-{name}"),
                &[set(2, operation, 0, 1, 0)],
                &[left, right],
                1,
            )?);
        }
    }
    for (name, left, right) in [
        ("overflow", 0x7fff_ffff, 2),
        ("min-negative", 0x8000_0000, u32::MAX),
        ("high-product", u32::MAX, u32::MAX),
    ] {
        cases.push(single(
            &format!("signed-multiply-{name}"),
            &[set(2, 4, 0, 1, 0)],
            &[left, right],
            2,
        )?);
    }
    for (name, records, parameters) in [
        (
            "conditional-undefined",
            vec![set(3, 0x80, 255, 255, 7), set(0, 0x80, 255, 255, 8)],
            vec![],
        ),
        (
            "conditional-defined",
            vec![set(0, 0x80, 255, 255, 8)],
            vec![3],
        ),
        ("undefined-source", vec![set(0, 1, 7, 8, 0)], vec![]),
        ("stage-bits", vec![set(0, 0, 0x84, 255, 0)], vec![]),
        (
            "optional-short-data",
            vec![vec![0x0d, 0, 0, 255, 255, 7, 8, 9]],
            vec![],
        ),
        ("unknown-op", vec![set(0, 0x40, 255, 255, 99)], vec![4]),
        ("left-shift-masked", vec![set(2, 5, 0, 1, 0)], vec![7, 33]),
        ("highest-target", vec![set(127, 0, 255, 255, 7)], vec![]),
    ] {
        cases.push(single(name, &records, &parameters, 2)?);
    }
    Ok(cases)
}
