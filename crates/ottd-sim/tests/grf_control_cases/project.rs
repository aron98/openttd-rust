//! Exact trace projection without fixture expectations.
use super::Result;
use super::{ControlLoadReport, FileControlState, LoadEvent, LoadStage, OverrideState};
use serde_json::{Value, json};
const fn stage(stage: LoadStage) -> u8 {
    match stage {
        LoadStage::FileScan => 0,
        LoadStage::SafetyScan => 1,
        LoadStage::LabelScan => 2,
        LoadStage::Init => 3,
        LoadStage::Reserve => 4,
        LoadStage::Activation => 5,
    }
}
fn files(files: &[FileControlState]) -> Value {
    json!(files.iter().map(|f|json!({"config_grfid":f.config_grfid,"file_grfid":f.file_grfid,"version":f.version,"status":format!("{:?}",f.status),"reserved":f.reserved,"parameters":f.parameters,
        "labels":f.labels.as_ref().map(|labels|labels.iter().map(|label|json!({"id":label.id,"line":label.line,"offset":label.offset})).collect::<Vec<_>>()),
        "errors":f.errors.iter().map(|e|json!({"failure":format!("{:?}",e.failure),"line":e.line})).collect::<Vec<_>>() })).collect::<Vec<_>>())
}
fn overrides(overrides: &[OverrideState]) -> Value {
    json!(overrides.iter().map(|entry|json!({"config_grfid":entry.config_grfid,"line":entry.line,"bytes":entry.bytes})).collect::<Vec<_>>())
}
/// Compare or project complete observable control state.
/// # Errors
/// Rejects malformed observations, unsupported baseline context, or mismatched state.
pub fn rust(report: &ControlLoadReport) -> Result<Value> {
    let mut events = Vec::new();
    let mut pending = None;
    for event in &report.events {
        match event {
            LoadEvent::StageStart(value)=>events.push(json!({"kind":"stage_start","stage":stage(*value)})),
            LoadEvent::StageEnd{stage:value,files:states,overrides:values}=>events.push(json!({"kind":"stage_end","stage":stage(*value),"files":files(states),"overrides":overrides(values)})),
            LoadEvent::Record{location,action,executed}=>pending=Some((location,action,executed)),
            LoadEvent::Decision{location,skip,next_line,next_offset,files:states,overrides:values}=>{
                let (original,action,executed)=pending.take().ok_or("decision without record")?;
                if original!=location {return Err("mismatched decision location".into());}
                events.push(json!({"kind":"record","stage":stage(location.stage),"file":location.file,"line":location.line,"offset":location.offset,"action":action,"executed":executed,
                    "skip":skip,"next_line":next_line,"next_offset":next_offset,"files":files(states),"overrides":overrides(values)}));
            }
            LoadEvent::Label{..}|LoadEvent::Parameters{..}|LoadEvent::Override{..}|LoadEvent::Jump{..}|LoadEvent::Status{..}=>(),
        }
    }
    if pending.is_some() {
        return Err("unobserved final record".into());
    }
    Ok(json!({"events":events,"files":files(&report.files),"overrides":[]}))
}
fn strip_baseline(value: &mut Value, prefix: &[u32]) -> Result {
    if let Some(states) = value.get_mut("files") {
        let states = states.as_array_mut().ok_or("files array")?;
        for (state, id) in states.iter().take(prefix.len()).zip(prefix) {
            if state.get("config_grfid") != Some(&json!(id)) {
                return Err("baseline config identity changed".into());
            }
            if state
                .get("file_grfid")
                .is_some_and(|v| !v.is_null() && v != &json!(id))
            {
                return Err("unsupported changed baseline dynamic identity".into());
            }
        }
        if states.len() < prefix.len() {
            return Err("baseline state missing".into());
        }
        states.drain(..prefix.len());
    }
    if let Some(values) = value.get_mut("overrides") {
        values
            .as_array_mut()
            .ok_or("overrides array")?
            .retain(|entry| {
                !prefix
                    .iter()
                    .any(|id| entry.get("config_grfid") == Some(&json!(id)))
            });
    }
    Ok(())
}
/// Compare or project complete observable control state.
/// # Errors
/// Rejects malformed observations, unsupported baseline context, or mismatched state.
pub fn native(observation: &Value, prefix: &[u32]) -> Result<Value> {
    let mut events = Vec::new();
    for value in observation
        .get("events")
        .and_then(Value::as_array)
        .ok_or("native events")?
    {
        let mut value = value.clone();
        if value.get("kind") == Some(&json!("record")) {
            let index = value
                .get("file")
                .and_then(Value::as_u64)
                .ok_or("file index")?;
            if index < u64::try_from(prefix.len())? {
                continue;
            }
            *value.get_mut("file").ok_or("file field")? = json!(
                index
                    .checked_sub(u64::try_from(prefix.len())?)
                    .ok_or("file normalization")?
            );
        }
        strip_baseline(&mut value, prefix)?;
        events.push(value);
    }
    let mut final_state = json!({"files":observation.get("files").ok_or("final files")?,"overrides":observation.get("overrides").ok_or("final overrides")?});
    strip_baseline(&mut final_state, prefix)?;
    final_state
        .as_object_mut()
        .ok_or("final object")?
        .insert("events".into(), json!(events));
    Ok(final_state)
}
/// Compare or project complete observable control state.
/// # Errors
/// Rejects malformed observations, unsupported baseline context, or mismatched state.
pub fn compare(native: &Value, rust: &Value) -> Result {
    if native != rust {
        return Err("original LoadNewGRF control trace mismatch".into());
    }
    Ok(())
}

pub(crate) fn field_pointer(value: &Value, key: &str, path: &str) -> Option<String> {
    find_field(value, key, path, true).or_else(|| find_field(value, key, path, false))
}
fn find_field(value: &Value, key: &str, path: &str, nonempty: bool) -> Option<String> {
    match value {
        Value::Object(fields) => {
            if fields.get(key).is_some_and(|v| {
                !nonempty || (!v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
            }) {
                return Some(format!("{path}/{key}"));
            }
            fields.iter().find_map(|(name, child)| {
                find_field(child, key, &format!("{path}/{name}"), nonempty)
            })
        }
        Value::Array(values) => values.iter().enumerate().find_map(|(index, child)| {
            find_field(child, key, &format!("{path}/{index}"), nonempty)
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}
pub(crate) fn changed(value: &Value) -> Value {
    match value {
        Value::Null => json!(0),
        Value::Bool(value) => json!(!value),
        Value::Number(value) => json!(value.as_i64().unwrap_or(0).wrapping_add(1)),
        Value::String(value) => json!(match value.as_str() {
            "Unknown" => "Disabled",
            "Disabled" => "Unknown",
            "Initialised" => "Activated",
            "Activated" => "Initialised",
            "ReadBounds" => "UnexpectedSprite",
            _ => "altered",
        }),
        Value::Array(values) => {
            let mut result = values.clone();
            if let Some(first) = result.first_mut() {
                *first = changed(first);
            } else {
                result.push(json!(0));
            }
            json!(result)
        }
        Value::Object(fields) => {
            let mut result = fields.clone();
            if let Some((_, first)) = result.iter_mut().next() {
                *first = changed(first);
            } else {
                result.insert("altered".into(), json!(true));
            }
            Value::Object(result)
        }
    }
}
