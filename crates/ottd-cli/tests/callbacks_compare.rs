//! Independent output mutations must be detected by the actual comparison command.
use serde_json::{Value, json};
use std::{fs, process::Command};

const CONTROLS: [(&str, &str, &str, &str); 9] = [
    (
        "vehicle_age",
        "calendar_subtypes_decay_wrap_sparse",
        "/callback/state/vehicles/0/age",
        "$.callback.state.vehicles[0].age",
    ),
    (
        "vehicle_profit",
        "economy_year_mixed_groups_thresholds",
        "/callback/state/vehicles/0/profit_last_year",
        "$.callback.state.vehicles[0].profit_last_year",
    ),
    (
        "group_profit",
        "economy_year_mixed_groups_thresholds",
        "/callback/state/groups/0/profit_last_year",
        "$.callback.state.groups[0].profit_last_year",
    ),
    (
        "house_age",
        "house_year_all_ages_raw_fields",
        "/callback/state/map/tiles/256/m5",
        "$.callback.state.map.tiles[256].m5",
    ),
    (
        "company_expenses",
        "company_year_signed_expenses",
        "/callback/state/companies/0/yearly_expenses/1/0",
        "$.callback.state.companies[0].yearly_expenses[1][0]",
    ),
    (
        "station_status",
        "station_month_all_status_bits",
        "/callback/state/stations/0/goods/0/status",
        "$.callback.state.stations[0].goods[0].status",
    ),
    (
        "industry_history",
        "industry_month_full_history_saturation",
        "/callback/state/industries/0/produced/0/history/1/production",
        "$.callback.state.industries[0].produced[0].history[1].production",
    ),
    (
        "random_state",
        "industry_month_full_history_saturation",
        "/callback/state/random_state/0",
        "$.callback.state.random_state[0]",
    ),
    (
        "callback_phase",
        "industry_month_year_before_rewind",
        "/callback/phase/year",
        "$.callback.phase.year",
    ),
];

#[test]
fn callback_output_mutations_report_exact_path_and_values() -> Result<(), Box<dyn std::error::Error>>
{
    // Given independently captured native outputs already replayed by the callback driver.
    let source = std::env::var("OTTD_CALLBACK_JSON").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference/callbacks.json"
        )
        .to_owned()
    });
    let corpus: Value = serde_json::from_reader(fs::File::open(source)?)?;
    let cases: Vec<_> = ["vehicle_cases", "periodic_cases", "industry_cases"]
        .into_iter()
        .map(|key| {
            corpus
                .get(key)
                .and_then(Value::as_array)
                .ok_or("missing cases")
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    let temp = tempfile::tempdir()?;
    let root = std::env::var_os("OTTD_CALLBACK_ARTIFACTS")
        .map_or_else(|| temp.path().to_path_buf(), std::path::PathBuf::from)
        .join("mismatch");
    fs::create_dir_all(&root)?;
    for (name, case_name, pointer, expected_path) in CONTROLS {
        let case = cases
            .iter()
            .find(|case| case.get("name").and_then(Value::as_str) == Some(case_name))
            .ok_or("missing control case")?;
        let expected = output_document(case)?;
        let expected_value = expected
            .pointer(pointer)
            .ok_or("missing control field")?
            .clone();
        let actual_value: Value = (expected_value
            .as_i64()
            .ok_or("control must be exact integer")?
            ^ 1)
        .into();
        let mut actual = expected.clone();
        *actual
            .pointer_mut(pointer)
            .ok_or("missing mutation field")? = actual_value.clone();
        let expected_file = root.join(format!("{name}.expected.json"));
        let actual_file = root.join(format!("{name}.actual.json"));
        fs::write(&expected_file, serde_json::to_vec(&expected)?)?;
        fs::write(&actual_file, serde_json::to_vec(&actual)?)?;
        let binary = env!("CARGO_BIN_EXE_ottd");
        let baseline = Command::new(binary)
            .arg("compare")
            .arg(&expected_file)
            .arg(&expected_file)
            .output()?;
        fs::write(root.join(format!("{name}.baseline.log")), &baseline.stdout)?;
        assert!(baseline.status.success());
        // When the real comparator receives one mutated output field.
        let output = Command::new(binary)
            .arg("compare")
            .arg(&expected_file)
            .arg(&actual_file)
            .output()?;
        fs::write(root.join(format!("{name}.stderr.log")), &output.stderr)?;
        fs::write(
            root.join(format!("{name}.command.json")),
            serde_json::to_vec(
                &json!({"binary":binary,"args":["compare",expected_file,actual_file]}),
            )?,
        )?;
        // Then rejection identifies the exact path and both integer values.
        assert!(!output.status.success(), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        assert_eq!(
            String::from_utf8(output.stderr)?,
            format!("Error: {expected_path}: expected {expected_value}, actual {actual_value}\n"),
            "{name}"
        );
    }
    Ok(())
}

fn output_document(case: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    let operation = case.get("operation").ok_or("missing operation")?;
    let kind = operation
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("missing kind")?;
    let state = case.get("after").ok_or("missing captured output")?;
    let callback = match kind {
        "calendar_day" | "economy_year" => {
            json!({"kind":"vehicle","operation":operation,"state":state})
        }
        "house_year" | "company_year" | "station_month" => json!({"kind":kind,"state":state}),
        "industry_month" => {
            json!({"kind":kind,"phase":operation.get("phase").ok_or("missing phase")?,"state":state})
        }
        _ => return Err("unknown captured callback".into()),
    };
    Ok(json!({"schema_version":1,"callback":callback}))
}
