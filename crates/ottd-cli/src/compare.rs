use anyhow::{Context, Result, ensure};
use serde::de::{Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

/// Reads a JSON document within a byte limit.
/// # Errors
/// Returns an error for unreadable, oversized or malformed input.
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
    serde_json::from_slice::<ExactValue>(&bytes)
        .map(|value| value.0)
        .with_context(|| format!("invalid JSON: {}", path.display()))
}

struct ExactValue(Value);

impl<'de> Deserialize<'de> for ExactValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_any(ExactVisitor)
    }
}

struct ExactVisitor;

impl<'de> Visitor<'de> for ExactVisitor {
    type Value = ExactValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON with unique keys and exact i64/u64 integer numbers")
    }

    fn visit_map<M: MapAccess<'de>>(
        self,
        mut map: M,
    ) -> std::result::Result<Self::Value, M::Error> {
        let mut object = serde_json::Map::new();
        while let Some((key, value)) = map.next_entry::<String, ExactValue>()? {
            if object.insert(key.clone(), value.0).is_some() {
                return Err(M::Error::custom(format!("duplicate object key: {key}")));
            }
        }
        Ok(ExactValue(Value::Object(object)))
    }

    fn visit_seq<S: SeqAccess<'de>>(
        self,
        mut seq: S,
    ) -> std::result::Result<Self::Value, S::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element::<ExactValue>()? {
            values.push(value.0);
        }
        Ok(ExactValue(Value::Array(values)))
    }

    fn visit_i64<E: Error>(self, value: i64) -> std::result::Result<Self::Value, E> {
        Ok(ExactValue(value.into()))
    }

    fn visit_u64<E: Error>(self, value: u64) -> std::result::Result<Self::Value, E> {
        Ok(ExactValue(value.into()))
    }

    fn visit_bool<E: Error>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(ExactValue(value.into()))
    }

    fn visit_str<E: Error>(self, value: &str) -> std::result::Result<Self::Value, E> {
        Ok(ExactValue(value.into()))
    }

    fn visit_unit<E: Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(ExactValue(Value::Null))
    }
}

/// Returns the first differing path and values, retaining unknown fields.
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
