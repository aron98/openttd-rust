//! Exact passive-observer shape from typed Rust caches and authoritative saved fields.
use super::Result;
use ottd_save::world::World;
use ottd_sim::runtime::rail::ConsistCaches;
use serde_json::{Value, json};

pub(super) fn snapshot(world: &World, caches: &ConsistCaches) -> Result<Value> {
    let saved = world.saved_json()?;
    let mut rails = Vec::new();
    for part in &caches.parts {
        let id = part.cache.id.raw();
        let train = saved
            .pointer(&format!("/chunks/VEHS/records/{id}/train/0"))
            .ok_or("saved train")?;
        let common = train.pointer("/common/0").ok_or("saved common")?;
        let Value::Object(mut actual) = serde_json::to_value(part)? else {
            return Err("part object".into());
        };
        for (key, field) in [
            ("engine_id", "engine_type"),
            ("tile", "tile"),
            ("subtype", "subtype"),
            ("capacity", "cargo_cap"),
            ("current_speed", "cur_speed"),
            ("acceleration", "acceleration"),
            ("vehstatus", "vehstatus"),
            ("owner", "owner"),
            ("cargo_type", "cargo_type"),
            ("cargo_subtype", "cargo_subtype"),
            ("sprite_number", "spritenum"),
            ("direction", "direction"),
            ("x", "x_pos"),
            ("y", "y_pos"),
            ("z", "z_pos"),
        ] {
            actual.insert(
                key.into(),
                common.get(field).ok_or("saved common field")?.clone(),
            );
        }
        for field in ["flags", "track"] {
            actual.insert(
                field.into(),
                train.get(field).ok_or("saved train field")?.clone(),
            );
        }
        for (key, link) in [
            ("previous", part.previous),
            ("next", part.next),
            ("other_multiheaded_part", part.other_multiheaded_part),
        ] {
            actual.insert(
                key.into(),
                json!(link.map_or(1_048_575, ottd_sim::runtime::VehicleId::raw)),
            );
        }
        let stored = common
            .get("cargo.action_counts")
            .and_then(Value::as_array)
            .ok_or("cargo actions")?
            .iter()
            .take(3)
            .map(|v| v.as_u64().ok_or("cargo count"))
            .collect::<std::result::Result<Vec<_>, _>>()?
            .iter()
            .sum::<u64>();
        actual.insert("stored_count".into(), json!(stored));
        rails.push(Value::Object(actual));
    }
    let date = saved.pointer("/chunks/DATE/records/0").ok_or("date")?;
    Ok(json!({"phase":"after-load-before-tick", "rail":rails,
        "random":[date.get("random_state[0]").ok_or("rng0")?,date.get("random_state[1]").ok_or("rng1")?],
        "tick":date.get("tick_counter").ok_or("tick")?}))
}
