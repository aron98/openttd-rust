//! Exact native foreach sessions use the existing passive array observer.
use super::{Result, replay};
use std::{fs, path::Path};

#[test]
fn native_foreach_sessions() -> Result {
    let corpus = corpus();
    let mut count = 0;
    for attempt in ["attempt-01", "attempt-02", "attempt-03", "attempt-04"] {
        let base = corpus.join(attempt);
        let mut members = fs::read_dir(base.join("fixtures"))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        members.sort();
        for path in members {
            let name = path.file_stem().and_then(|s| s.to_str()).ok_or("name")?;
            replay(&base, name)?;
            count += 1;
        }
    }
    assert_eq!(count, 135);
    Ok(())
}

#[test]
fn native_byte_string_foreach_sessions() -> Result {
    let base = corpus().join("attempt-05");
    let mut members = fs::read_dir(base.join("fixtures"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    members.sort();
    assert_eq!(members.len(), 120);
    for path in members {
        let name = path.file_stem().and_then(|s| s.to_str()).ok_or("name")?;
        replay(&base, name)?;
    }
    Ok(())
}

fn corpus() -> std::path::PathBuf {
    std::env::var_os("OTTD_SCRIPT_FOREACH_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/foreach-native"),
        std::path::PathBuf::from,
    )
}
