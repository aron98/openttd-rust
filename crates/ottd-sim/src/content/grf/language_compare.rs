use super::{Result, project};
use serde_json::{Value, json};
use std::path::Path;

fn pointers(value: &Value, prefix: &str, output: &mut Vec<String>) {
    match value {
        Value::Object(fields) if !fields.is_empty() => {
            for (name, value) in fields {
                pointers(
                    value,
                    &format!("{prefix}/{}", name.replace('~', "~0").replace('/', "~1")),
                    output,
                );
            }
        }
        Value::Array(values) if !values.is_empty() => {
            if values.iter().all(Value::is_number) {
                output.push(format!("{prefix}/0"));
                if values.len() > 1 {
                    output.push(format!("{prefix}/{}", values.len().saturating_sub(1)));
                }
            } else {
                for (index, value) in values.iter().enumerate() {
                    pointers(value, &format!("{prefix}/{index}"), output);
                }
            }
        }
        Value::Null
        | Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::Array(_)
        | Value::Object(_) => output.push(prefix.into()),
    }
}

pub(super) fn checked(directory: &Path, name: &str, pair: (&Value, &Value)) -> Result {
    let (native, rust) = pair;
    project::compare(native, rust)?;
    let mut targets = Vec::new();
    pointers(native, "", &mut targets);
    let mut controls = Vec::new();
    for pointer in targets {
        let mut altered = native.clone();
        let field = altered.pointer_mut(&pointer).ok_or("control pointer")?;
        let before = field.clone();
        *field = project::changed(field);
        let after = field.clone();
        if project::compare(&altered, rust).is_ok() {
            return Err(format!("accepted mutation {name}{pointer}").into());
        }
        controls.push(json!({"pointer":pointer,"before":before,"after":after,"rejected":true}));
    }
    if let Some(values) = native.as_array() {
        let mut altered = values.clone();
        altered.push(Value::Null);
        if project::compare(&json!(altered), rust).is_ok() {
            return Err("accepted extra observation".into());
        }
        controls.push(json!({"operation":"append-null","rejected":true}));
    }
    std::fs::write(
        directory.join(format!("{name}-controls.json")),
        serde_json::to_vec(&controls)?,
    )?;
    Ok(())
}
