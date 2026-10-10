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
            if [
                "string-iterable-0-buffer",
                "string-iterable-0-unsigned",
                "string-iterable-0-utf8",
            ]
            .contains(&name)
            {
                continue;
            }
            replay(&base, name)?;
            count += 1;
        }
    }
    assert_eq!(count, 129);
    Ok(())
}

#[test]
fn native_string_iterable_compiles_exactly_but_stops_at_typed_boundary() -> Result {
    let base = corpus().join("boundaries");
    for attempt in ["attempt-01", "attempt-02"] {
        for feed in ["buffer", "unsigned", "utf8"] {
            let bytes = fs::read(base.join("string-iterable.nut"))?;
            let bytes = if feed == "unsigned" {
                bytes
                    .into_iter()
                    .map(char::from)
                    .collect::<String>()
                    .into_bytes()
            } else {
                bytes
            };
            let native = fs::read_to_string(base.join(format!("{attempt}-{feed}.txt")))?;
            let compiled = native
                .split_once("compiled\n")
                .ok_or("native compilation")?
                .1
                .lines()
                .take_while(|line| {
                    line.starts_with("stack ")
                        || line.starts_with("literal ")
                        || line.starts_with("op ")
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            let program = crate::compile_bytes(&bytes)?;
            let mut rust = String::new();
            super::dump(&program, &mut rust)?;
            assert_eq!(rust, compiled, "{attempt}-{feed}");
            assert_eq!(
                crate::Vm::new(&program)?.resume(10000),
                Err(crate::VmError::UnsupportedReceiver)
            );
        }
    }
    Ok(())
}

fn corpus() -> std::path::PathBuf {
    std::env::var_os("OTTD_SCRIPT_FOREACH_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/foreach-native"),
        std::path::PathBuf::from,
    )
}
