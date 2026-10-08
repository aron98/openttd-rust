//! Optional runtime oracle check; paths identify independently generated artifacts.
use ottd_save::{DEFAULT_MAX_BYTES, Savegame, WorldSnapshot};

#[test]
#[ignore = "requires OTTD_ORACLE_SAVE and OTTD_ORACLE_JSON from the instrumented native engine"]
fn matches_independent_runtime_snapshot() {
    // Given the save and runtime snapshot from exactly the same native saving boundary.
    let save_path = std::env::var("OTTD_ORACLE_SAVE").unwrap();
    let json_path = std::env::var("OTTD_ORACLE_JSON").unwrap();
    let save_bytes = std::fs::read(save_path).unwrap();
    let expected: serde_json::Value =
        serde_json::from_slice(&std::fs::read(json_path).unwrap()).unwrap();
    // When the save's typed state is decoded independently.
    let actual = Savegame::decode(&save_bytes, DEFAULT_MAX_BYTES)
        .unwrap()
        .snapshot()
        .unwrap();
    // Then every JSON field matches, with no filtering or discarded unknown fields.
    assert_eq!(serde_json::to_value(&actual).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<WorldSnapshot>(expected).unwrap(),
        actual
    );
}
