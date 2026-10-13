// Shared strict comparison of every native owner slot and every raw score component.
fn compare_raw_ratings(runtime: &SimulationRuntime, native: &Path, label: &str) -> Result {
    let rating: Value = serde_json::from_slice(&std::fs::read(native.join(format!("{label}.rating.json")))?)?;
    let rows = rating.get("scores").and_then(Value::as_array).ok_or("native score slots")?;
    assert_eq!(rows.len(), 15);
    let mut expected = Vec::new();
    for (owner, row) in rows.iter().enumerate() {
        assert_eq!(row.get("owner"), Some(&json!(owner)));
        expected.push(serde_json::from_value::<[i64; 10]>(row.get("parts").ok_or("raw parts")?.clone())?);
    }
    let expected: [[i64; 10]; 15] = expected.try_into().map_err(|_| "native owner count")?;
    assert_eq!(runtime.company_rating_caches().parts(), &expected, "{label}: all 150 raw score slots");
    Ok(())
}
