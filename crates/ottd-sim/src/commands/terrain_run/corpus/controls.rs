use super::{Mismatch, PathBuf, Result, corpus, io, replay};
use serde_json::json;

#[test]
#[ignore = "requires frozen native24 controls and a fresh TREE_CORPUS_OUTPUT directory"]
fn native_trace_corruptions_fail_actual_rust_comparisons() -> Result {
    let directory = corpus()?;
    let controls = [
        "change-flags",
        "change-test-rating",
        "drop-scope",
        "duplicate-exec-penalty",
        "missing-failed-step",
        "modify-pass0",
        "reset-nested-map",
        "set-test-have-rating",
        "swap-tree-order",
    ];
    for name in controls {
        let control = directory.join("controls").join(name);
        let baseline = PathBuf::from(std::fs::read_to_string(control.join("baseline.txt"))?.trim());
        replay::run(&baseline, true, &format!("control-{name}-baseline"))?;
        let error = replay::run(&control, true, &format!("control-{name}-mutated"))
            .err()
            .ok_or("mutated expected native trace unexpectedly matched Rust")?;
        let mismatch = error
            .downcast_ref::<Mismatch>()
            .ok_or("control failed before semantic comparison")?;
        assert_eq!(mismatch.component, "trace", "{name}: {error}");
        io::record(
            &format!("control-{name}-rejection"),
            &json!({"control": name, "error": error.to_string()}),
        )?;
        replay::run(&baseline, true, &format!("control-{name}-restored"))?;
    }
    println!("NATIVE_CORPUS_CONTROLS_REJECTED count=9");
    Ok(())
}
