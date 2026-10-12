// Exact extraction of the explicitly owned three native profit fields.
type NativeProfitResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn profit(value: &serde_json::Value) -> NativeProfitResult<serde_json::Value> {
    let mut fields = serde_json::Map::new();
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
    Ok(serde_json::Value::Object(fields))
}

/// Extract the three owned profit fields without calculating native cache values.
///
/// # Errors
/// Returns an error when required native fields or four vehicle-type slots are missing.
fn owned_native(value: &serde_json::Value) -> NativeProfitResult<serde_json::Value> {
    let mut companies = serde_json::Map::new();
    for row in value["companies"].as_array().ok_or("native companies")? {
        let mut groups = serde_json::Map::new();
        for name in ["all", "default"] {
            let values = row[name].as_array().ok_or("native company groups")?;
            if values.len() != 4 {
                return Err("native company requires four vehicle-type slots".into());
            }
            groups.insert(
                name.into(),
                serde_json::Value::Array(values.iter().map(profit).collect::<NativeProfitResult<_>>()?),
            );
        }
        companies.insert(
            row["id"].as_u64().ok_or("native company ID")?.to_string(),
            serde_json::Value::Object(groups),
        );
    }
    let mut named = serde_json::Map::new();
    for row in value["groups"].as_array().ok_or("native named groups")? {
        named.insert(
            row["id"].as_u64().ok_or("native group ID")?.to_string(),
            serde_json::json!({
                "owner": row["owner"], "vehicle_type": row["type"], "parent": row["parent"],
                "profit": profit(&row["statistics"])?,
            }),
        );
    }
    Ok(serde_json::json!({"companies": companies, "named": named}))
}
