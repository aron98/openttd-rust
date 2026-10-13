use super::{decrement, scale};
use serde::Deserialize;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};

#[derive(Deserialize)]
struct Probe {
    width: u32,
    height: u32,
    old: u8,
    next: u8,
    underflow: bool,
    scale1: u32,
    scale16: u32,
}

#[test]
fn native_byte_equality_counts_as_underflow_at_maximum_map() {
    assert_eq!(scale(4096, 4096, 1), 256);
    for old in 0_u8..=255 {
        let (next, underflow) = decrement(old, scale(4096, 4096, 1));
        assert_eq!(next, old);
        assert!(underflow);
    }
    assert_eq!(scale(512, 512, 1), 4);
    assert_eq!(scale(64, 64, 16), 1);
}

#[test]
#[ignore = "requires separately compiled original source arithmetic probe, not a full-world claim"]
fn source_bound_native_counter_arithmetic() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::var("OTTD_TREE_COUNTER_PROBE")?;
    let mut rows = 0;
    for line in BufReader::new(File::open(path)?).lines() {
        let row: Probe = serde_json::from_str(&line?)?;
        assert_eq!(scale(row.width, row.height, 1), row.scale1);
        assert_eq!(scale(row.width, row.height, 16), row.scale16);
        let (next, underflow) = decrement(row.old, scale(row.width, row.height, 1));
        assert_eq!(next, row.next);
        assert_eq!(underflow, row.underflow);
        rows += 1;
    }
    assert_eq!(rows, 7 * 7 * 256);
    Ok(())
}
