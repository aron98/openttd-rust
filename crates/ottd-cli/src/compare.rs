use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

pub fn load_json(path: &Path, limit: usize) -> Result<Value> {
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(u64::try_from(limit)?.saturating_add(1))
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "JSON exceeds {limit} byte limit: {}",
        path.display()
    );
    serde_json::from_slice(&bytes).with_context(|| format!("invalid JSON: {}", path.display()))
}

pub fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, value) in left {
                let child = format!("{path}.{key}");
                let Some(other) = right.get(key) else {
                    return Some(format!("{child}: expected {value}, actual <missing>"));
                };
                if let Some(difference) = first_difference(value, other, &child) {
                    return Some(difference);
                }
            }
            for (key, value) in right {
                if !left.contains_key(key) {
                    return Some(format!("{path}.{key}: expected <missing>, actual {value}"));
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            for (index, (value, other)) in left.iter().zip(right).enumerate() {
                if let Some(difference) =
                    first_difference(value, other, &format!("{path}[{index}]"))
                {
                    return Some(difference);
                }
            }
            let index = left.len().min(right.len());
            if left.len() == right.len() {
                None
            } else {
                Some(format!(
                    "{path}[{index}]: expected {}, actual {}",
                    left.get(index)
                        .map_or_else(|| "<missing>".to_owned(), ToString::to_string),
                    right
                        .get(index)
                        .map_or_else(|| "<missing>".to_owned(), ToString::to_string)
                ))
            }
        }
        _ if expected == actual => None,
        _ => Some(format!("{path}: expected {expected}, actual {actual}")),
    }
}
