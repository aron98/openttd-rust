//! Original-engine command receipt and complete saved-state comparisons.
use ottd_save::{Savegame, world::World};
use ottd_sim::{CommandRequest, execute_command};
use serde_json::Value;

#[test]
#[ignore = "requires captured native replay; exercised by stage-3 driver"]
fn native_command_checkpoints_match() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(std::env::var("OTTD_COMMAND_NATIVE_DIR")?);
    let bytes = std::fs::read(directory.join("initial.sav"))?;
    let mut world = World::decode(&Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
    let actions: Value = serde_json::from_slice(&std::fs::read(directory.join("actions.json"))?)?;
    let native: Value = serde_json::from_slice(&std::fs::read(directory.join("results.json"))?)?;
    let actions = actions
        .get("actions")
        .and_then(Value::as_array)
        .ok_or("actions")?;
    let receipts = native
        .get("actions")
        .and_then(Value::as_array)
        .ok_or("receipts")?;
    let mut command_count = 0_u32;
    let mut checkpoint_count = 0_u32;
    for action in actions {
        match action
            .get("op")
            .and_then(Value::as_str)
            .ok_or("operation")?
        {
            "command" => {
                command_count = command_count
                    .checked_add(1)
                    .ok_or("command count overflow")?;
                let request: CommandRequest =
                    serde_json::from_value(action.get("request").ok_or("request")?.clone())?;
                let receipt = execute_command(&mut world, &request)?;
                let expected = receipts
                    .iter()
                    .find(|row| row.get("ordinal") == action.get("ordinal"))
                    .and_then(|row| row.get("receipt"))
                    .ok_or("native receipt")?;
                assert_eq!(
                    &serde_json::to_value(receipt)?,
                    expected,
                    "receipt at {:?}",
                    action.get("ordinal")
                );
            }
            "checkpoint" => {
                checkpoint_count = checkpoint_count
                    .checked_add(1)
                    .ok_or("checkpoint count overflow")?;
                let label = action.get("label").and_then(Value::as_str).ok_or("label")?;
                if label.is_empty()
                    || !label
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                {
                    return Err("unsafe checkpoint label".into());
                }
                let expected: Value = serde_json::from_slice(&std::fs::read(
                    directory.join(format!("{label}.world.json")),
                )?)?;
                let actual = world.saved_json()?;
                std::fs::write(
                    directory.join(format!("{label}.rust.world.json")),
                    serde_json::to_vec(&actual)?,
                )?;
                assert_eq!(actual, expected, "saved checkpoint {label}");
            }
            "tick" => return Err("command-only comparison cannot execute world ticks".into()),
            _ => return Err("unknown replay operation".into()),
        }
    }
    assert!(
        command_count > 0 && checkpoint_count > 0,
        "native command/checkpoint evidence missing"
    );
    println!(
        "Compared {command_count} native command receipts and {checkpoint_count} complete checkpoints"
    );
    Ok(())
}
