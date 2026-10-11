//! Array-aware native host sessions replayed through the production VM.
use super::root_sessions::{bytes, dump};
use super::{Execution, Vm};
use crate::{Value, VmError};
use std::{error::Error, fmt::Write, fs, path::Path};
mod commands;
mod execution;
mod foreach;
mod state;
use state::Session;
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
fn native_projection(text: &str) -> Result<String> {
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("collected ") {
            assert_eq!(
                line, "collected 0",
                "only an observed no-op collection is projected away"
            );
            continue;
        }
        if line.starts_with("stack_top ")
            || line.starts_with("array_refs ")
            || line.starts_with("external_")
            || line.starts_with("vm_root ")
        {
            continue;
        }
        if line.starts_with("compile_error ") {
            writeln!(out, "compile_error")?;
        } else if line.starts_with("runtime_error ") {
            let f: Vec<_> = line.split_whitespace().collect();
            let message = bytes(f.last().ok_or("error bytes")?)?;
            let error = if message.starts_with(b"the index ") {
                VmError::MissingIndex
            } else if message == b"division by zero" {
                VmError::DivisionByZero
            } else if message == b"indexing array with integer" || message == b"cannot iterate null"
            {
                VmError::OperandType
            } else {
                return Err(format!("unclassified native error {message:?}").into());
            };
            writeln!(out, "runtime_error {} {error:?}", f.get(1).ok_or("debt")?)?;
        } else if line.starts_with("host_error ") {
            let f: Vec<_> = line.split_whitespace().collect();
            let message = bytes(f.last().ok_or("host bytes")?)?;
            assert!(message.starts_with(b"the index "));
            writeln!(out, "host_error MissingIndex")?;
        } else {
            writeln!(out, "{line}")?;
        }
    }
    Ok(out)
}
fn corpus() -> std::path::PathBuf {
    std::env::var_os("OTTD_SCRIPT_ARRAY_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/arrays-native"),
        std::path::PathBuf::from,
    )
}
fn replay(base: &Path, name: &str) -> Result {
    let input = fs::read_to_string(base.join("fixtures").join(format!("{name}.session")))?;
    let native = fs::read_to_string(base.join("native").join(format!("{name}.txt")))?;
    let mut out = String::from("configured_empty_roots 0 0 1\n");
    let mut session = Session::new();
    for (step, line) in input
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .enumerate()
    {
        writeln!(out, "step {step} {line}")?;
        session.command(line, base, &mut out)?;
    }
    assert_eq!(out, native_projection(&native)?, "{name}");
    Ok(())
}
#[test]
fn actual_native_owned_array_sessions() -> Result {
    let base = corpus();
    for feed in ["buffer", "unsigned", "utf8"] {
        for family in [
            "candidate-scores",
            "rolling-history",
            "persistent-alias",
            "empty-truth-type",
            "fresh-allocation",
            "identity",
            "literal-folding",
            "indexed-alias-order",
            "defined-float-index",
            "after-write-failure",
            "host-interleave",
            "temporary-only",
            "newslot-array-error",
            "float-cast-boundary",
            "legacy-boundary-promotion",
            "host-construction",
            "shared-independent",
            "child-temporary",
            "index-error-order",
            "string-bytes",
            "malformed-array",
            "lazy-array",
            "partial-initialization",
        ] {
            replay(&base.join("attempt-01"), &format!("{family}-{feed}"))?;
        }
        for family in ["all-construction-boundaries", "set-before-after"] {
            replay(&base.join("attempt-02"), &format!("{family}-{feed}"))?;
        }
        replay(
            &base.join("attempt-03"),
            &format!("newslot-persistence-{feed}"),
        )?;
    }
    Ok(())
}

#[test]
fn native_valid_object_boundaries_keep_exact_compilation() -> Result {
    enum Outcome {
        Array,
        Integer(i64),
        Error(VmError),
    }
    use Outcome::{Array, Error, Integer};
    let base = corpus().join("attempt-01");
    for feed in ["buffer", "unsigned", "utf8"] {
        for (family, bodies) in [
            (
                "nested-and-cycle",
                vec![("nested-and-cycle-0", Array), ("nested-and-cycle-1", Array)],
            ),
            ("cycle-collection", vec![("native-cycle", Integer(0))]),
            (
                "ordering-conversion",
                vec![
                    (
                        "ordering-conversion-0",
                        Error(VmError::UnsupportedArrayOrdering),
                    ),
                    (
                        "ordering-conversion-1",
                        Error(VmError::UnsupportedRuntimeValue),
                    ),
                ],
            ),
            (
                "delegate-and-missing",
                vec![
                    (
                        "delegate-and-missing-0",
                        Error(VmError::UnsupportedRuntimeValue),
                    ),
                    ("delegate-and-missing-1", Error(VmError::MissingIndex)),
                    ("delegate-and-missing-2", Error(VmError::MissingIndex)),
                ],
            ),
        ] {
            let native =
                fs::read_to_string(base.join("native").join(format!("{family}-{feed}.txt")))?;
            let blocks: Vec<_> = native
                .split("compiled\n")
                .skip(1)
                .map(|part| {
                    part.lines()
                        .take_while(|line| {
                            line.starts_with("stack ")
                                || line.starts_with("literal ")
                                || line.starts_with("op ")
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n"
                })
                .collect();
            let mut bodies = bodies;
            if family == "nested-and-cycle" {
                bodies.push(("scalar-return", Integer(0)));
            }
            assert_eq!(blocks.len(), bodies.len());
            for ((name, expected), block) in bodies.into_iter().zip(blocks) {
                let source = fs::read(base.join("fixtures").join(format!("{name}.nut")))?;
                let source = if feed == "unsigned" {
                    source
                        .into_iter()
                        .map(char::from)
                        .collect::<String>()
                        .into_bytes()
                } else {
                    source
                };
                let program = crate::compile_bytes(&source)?;
                let mut compiled = String::new();
                dump(&program, &mut compiled)?;
                assert_eq!(compiled, block, "{family}-{feed}/{name}");
                let result = Vm::new(&program)?.resume(10_000);
                match expected {
                    Array => assert!(
                        matches!(result, Ok(Execution::Returned(Value::Array(_)))),
                        "{name}"
                    ),
                    Integer(value) => assert_eq!(
                        result,
                        Ok(Execution::Returned(Value::Integer(value))),
                        "{name}"
                    ),
                    Error(error) => assert_eq!(result, Err(error), "{name}"),
                }
            }
        }
    }
    Ok(())
}
