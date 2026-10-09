//! Full native state-loop saved-world checkpoint comparisons.
use ottd_save::{Savegame, world::World};
use ottd_sim::{CommandRequest, advance_world, execute_command};
use serde_json::Value;

#[test]
#[ignore = "requires captured native StateGameLoop replay; exercised by stage-3 driver"]
fn native_tick_checkpoints_match() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(std::env::var("OTTD_TICK_NATIVE_DIR")?);
    let mut world = World::decode(&Savegame::decode(
        &std::fs::read(directory.join("initial.sav"))?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let document: Value = serde_json::from_slice(&std::fs::read(directory.join("actions.json"))?)?;
    let actions = document
        .get("actions")
        .and_then(Value::as_array)
        .ok_or("actions")?;
    let mut calls = 0_u64;
    let mut checkpoints = 0_u32;
    for action in actions {
        match action
            .get("op")
            .and_then(Value::as_str)
            .ok_or("operation")?
        {
            "tick" => {
                let count =
                    u32::try_from(action.get("count").and_then(Value::as_u64).ok_or("count")?)?;
                advance_world(&mut world, count)?;
                calls = calls.saturating_add(u64::from(count));
            }
            "command" => {
                let request: CommandRequest =
                    serde_json::from_value(action.get("request").ok_or("command")?.clone())?;
                execute_command(&mut world, &request)?;
            }
            "checkpoint" => {
                let label = action.get("label").and_then(Value::as_str).ok_or("label")?;
                if label.is_empty()
                    || !label
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                {
                    return Err("invalid label".into());
                }
                let expected: Value = serde_json::from_slice(&std::fs::read(
                    directory.join(format!("{label}.world.json")),
                )?)?;
                let actual = world.saved_json()?;
                std::fs::write(
                    directory.join(format!("{label}.rust.world.json")),
                    serde_json::to_vec(&actual)?,
                )?;
                let different: Vec<_> = actual
                    .get("chunks")
                    .and_then(Value::as_object)
                    .ok_or("chunks")?
                    .iter()
                    .filter_map(|(id, value)| {
                        (expected.get("chunks").and_then(|chunks| chunks.get(id)) != Some(value))
                            .then_some(id)
                    })
                    .collect();
                assert!(
                    actual == expected,
                    "checkpoint {label}: differing chunks {different:?}"
                );
                checkpoints = checkpoints.saturating_add(1);
            }
            _ => return Err("unknown operation".into()),
        }
    }
    assert!(calls > 0 && checkpoints > 0, "native tick evidence missing");
    println!(
        "Compared {calls} native StateGameLoop calls and {checkpoints} complete saved checkpoints"
    );
    Ok(())
}
