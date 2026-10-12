//! Actual original afterload observations of the three owned group-profit fields.
use ottd_save::{Savegame, world::World};
use ottd_sim::runtime::SimulationRuntime;
use serde_json::{Map, Value, json};
use std::{fs, path::PathBuf};
#[path = "industry_recipient_support/native_views.rs"]
mod native_views;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn profit(value: &Value) -> Result<Value> {
    let mut fields = Map::new();
    for field in [
        "profit_last_year",
        "profit_last_year_min_age",
        "num_vehicle_min_age",
    ] {
        fields.insert(
            field.into(),
            value
                .get(field)
                .ok_or("missing native profit field")?
                .clone(),
        );
    }
    Ok(Value::Object(fields))
}

fn owned_native(value: &Value) -> Result<Value> {
    let mut companies = Map::new();
    for row in value["companies"].as_array().ok_or("native companies")? {
        let mut groups = Map::new();
        for name in ["all", "default"] {
            let values = row[name].as_array().ok_or("native company groups")?;
            assert_eq!(values.len(), 4);
            groups.insert(
                name.into(),
                Value::Array(values.iter().map(profit).collect::<Result<_>>()?),
            );
        }
        companies.insert(
            row["id"].as_u64().ok_or("native company ID")?.to_string(),
            Value::Object(groups),
        );
    }
    let mut named = Map::new();
    for row in value["groups"].as_array().ok_or("native named groups")? {
        named.insert(
            row["id"].as_u64().ok_or("native group ID")?.to_string(),
            json!({
                "owner": row["owner"], "vehicle_type": row["type"], "parent": row["parent"],
                "profit": profit(&row["statistics"])?,
            }),
        );
    }
    Ok(json!({"companies": companies, "named": named}))
}

#[test]
#[ignore = "requires actual native afterload group snapshots and all prior cache views"]
fn restored_group_profits_match_original_afterload() -> Result {
    let native = PathBuf::from(std::env::var("INDUSTRY_RECIPIENT_NATIVE")?);
    let output = PathBuf::from(std::env::var("VEHICLE_YEAR_OUTPUT")?);
    fs::create_dir(&output)?;
    let input = fs::read(native.join("initial.sav"))?;
    let runtime = SimulationRuntime::restore_vanilla(World::decode(&Savegame::decode(
        &input,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)?;
    native_views::compare(&runtime, &output, "initial")?;
    let expected: Value = serde_json::from_slice(&fs::read(native.join("initial.groups.json"))?)?;
    let actual = serde_json::to_value(runtime.group_profit_caches())?;
    fs::write(
        output.join("actual-owned-profits.json"),
        serde_json::to_vec_pretty(&actual)?,
    )?;
    fs::write(
        output.join("native-full-groups.json"),
        serde_json::to_vec_pretty(&expected)?,
    )?;
    assert_eq!(
        actual,
        owned_native(&expected)?,
        "all three owned fields for every native company/type/group"
    );
    assert_eq!(
        runtime.saved_json()?,
        World::decode(&Savegame::decode(&input, ottd_save::DEFAULT_MAX_BYTES)?)?.saved_json()?
    );
    Ok(())
}
