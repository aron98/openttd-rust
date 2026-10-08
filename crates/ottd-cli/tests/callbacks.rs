//! Public command boundary for actual supported object callbacks.
use std::process::Command;

#[test]
fn callback_command_is_exposed_with_explicit_scope() -> Result<(), Box<dyn std::error::Error>> {
    // Given the built user-facing binary.
    let binary = env!("CARGO_BIN_EXE_ottd");
    // When the supported callback command is requested.
    let output = Command::new(binary)
        .args(["simulate-callbacks", "--help"])
        .output()?;
    // Then it is a callable command rather than a library-only implementation.
    assert!(output.status.success());
    Ok(())
}

#[test]
fn native_callbacks_match_through_cli_with_rejection_controls()
-> Result<(), Box<dyn std::error::Error>> {
    // Given the independent native callback corpus.
    let source = std::env::var("OTTD_CALLBACK_JSON").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference/callbacks.json"
        )
        .to_owned()
    });
    let corpus: serde_json::Value = serde_json::from_reader(std::fs::File::open(source)?)?;
    let vehicle_cases = corpus
        .get("vehicle_cases")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing cases")?;
    let periodic_cases = corpus
        .get("periodic_cases")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing periodic cases")?;
    let cases: Vec<_> = vehicle_cases.iter().chain(periodic_cases).collect();
    assert!(vehicle_cases.len() >= 12);
    assert_eq!(periodic_cases.len(), 8);
    let temp = tempfile::tempdir()?;
    let root = std::env::var_os("OTTD_CALLBACK_ARTIFACTS")
        .map_or_else(|| temp.path().to_path_buf(), std::path::PathBuf::from);
    std::fs::create_dir_all(&root)?;
    for case in &cases {
        let name = case
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or("missing name")?;
        let request = request_for(case, "before")?;
        let expected = request_for(case, "after")?;
        let path = root.join(format!("{name}.input.json"));
        std::fs::write(&path, serde_json::to_vec(&request)?)?;
        // When the public command executes the original callback boundary twice.
        let mut previous = None;
        for run in 0..2 {
            let output = Command::new(env!("CARGO_BIN_EXE_ottd"))
                .arg("simulate-callbacks")
                .arg(&path)
                .output()?;
            std::fs::write(
                root.join(format!("{name}.{run}.output.json")),
                &output.stdout,
            )?;
            std::fs::write(
                root.join(format!("{name}.{run}.stderr.log")),
                &output.stderr,
            )?;
            // Then every native field matches and repeated output is byte-identical.
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&output.stdout)?,
                expected,
                "{name}"
            );
            if let Some(bytes) = previous {
                assert_eq!(output.stdout, bytes);
            }
            previous = Some(output.stdout);
        }
    }
    // Given malformed, unknown, and unsupported boundary requests.
    let case = cases.first().ok_or("missing case")?;
    let mut unsupported = serde_json::json!({"schema_version":1,"callback":{"kind":"vehicle",
        "operation":{"kind":"economy_year"},"state":case.get("before").ok_or("missing before")?}});
    let state = unsupported
        .get_mut("callback")
        .and_then(|v| v.get_mut("state"))
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("missing state")?;
    state.insert("old_vehicle_warn".to_owned(), true.into());
    for (name, input) in [
        ("malformed", "{".to_owned()),
        (
            "unknown",
            "{\"schema_version\":1,\"callback\":{\"kind\":\"movement\"}}".to_owned(),
        ),
        ("unsupported_news", unsupported.to_string()),
    ] {
        let path = root.join(format!("{name}.input.json"));
        std::fs::write(&path, input)?;
        // When the binary reads an unsupported request.
        let output = Command::new(env!("CARGO_BIN_EXE_ottd"))
            .arg("simulate-callbacks")
            .arg(&path)
            .output()?;
        std::fs::write(root.join(format!("{name}.stderr.log")), &output.stderr)?;
        // Then failure never prints a successful state.
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    Ok(())
}

fn request_for(
    case: &serde_json::Value,
    phase: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let operation = case.get("operation").ok_or("missing operation")?;
    let kind = operation
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing kind")?;
    let state = case.get(phase).ok_or("missing state")?;
    let callback = match kind {
        "house_year" | "company_year" | "station_month" => {
            serde_json::json!({"kind":kind,"state":state})
        }
        "calendar_day" | "economy_year" => {
            serde_json::json!({"kind":"vehicle","operation":operation,"state":state})
        }
        _ => return Err("unsupported oracle operation".into()),
    };
    Ok(serde_json::json!({"schema_version":1,"callback":callback}))
}
