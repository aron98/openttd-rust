//! Negative controls over an actual upstream snapshot, run by check-snapshots.sh.
use std::{fs, path::PathBuf, process::Command};

#[test]
#[ignore = "requires OTTD_SNAPSHOT_DIR from the upstream differential harness"]
fn upstream_snapshot_negative_controls() {
    let dir = PathBuf::from(std::env::var_os("OTTD_SNAPSHOT_DIR").unwrap());
    let expected = dir.join("snapshot.json");
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(&expected).unwrap()).unwrap();
    for (name, pointer, diagnostic) in [
        ("tile", "/map/tiles/0/height", "$.map.tiles[0].height"),
        ("date", "/date/date", "$.date.date"),
        ("rng", "/date/random_state/0", "$.date.random_state[0]"),
        (
            "settings",
            "/settings/construction.map_height_limit",
            "$.settings.construction.map_height_limit",
        ),
    ] {
        let mut changed = original.clone();
        let field = changed.pointer_mut(pointer).unwrap();
        *field = serde_json::Value::from(field.as_u64().unwrap().wrapping_add(1));
        let actual = dir.join(format!("mutated-{name}.json"));
        fs::write(&actual, serde_json::to_vec(&changed).unwrap()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_ottd"))
            .arg("compare")
            .arg(&expected)
            .arg(&actual)
            .output()
            .unwrap();
        fs::write(dir.join(format!("negative-{name}.log")), &output.stderr).unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        assert!(String::from_utf8_lossy(&output.stderr).contains("expected"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("actual"));
    }
}
