use super::{Pack, Result, compare, field, project, text_cases};
use crate::content::grf;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn state(
    directory: &Path,
    native: &Value,
    expected: (&grf::load_language_state::LanguageReport, &[u32]),
) -> Result {
    let (language, prefix) = expected;
    let pack = language
        .catalog
        .iter()
        .find(|pack| pack.language == language.selected)
        .ok_or("selected pack")?;
    compare::checked(
        directory,
        "selected",
        (
            field(native, "/catalog/selected")?,
            &json!(language.selected),
        ),
    )?;
    compare::checked(
        directory,
        "selected-header",
        (field(native, "/catalog/selected_header")?, &json!(pack)),
    )?;
    let strings = pack.tables.iter().fold(0_usize, |count, value| {
        count.saturating_add(usize::from(*value))
    });
    compare::checked(
        directory,
        "selected-strings",
        (field(native, "/catalog/selected_strings")?, &json!(strings)),
    )?;
    for (field_name, expected) in [
        ("pack_version", u64::from(grf::language_pack::PACK_VERSION)),
        ("plural_rules", u64::from(grf::language_pack::PLURAL_RULES)),
        ("plural_forms", u64::from(grf::language_pack::PLURAL_FORMS)),
    ] {
        compare::checked(
            directory,
            field_name,
            (
                field(native, &format!("/catalog/{field_name}"))?,
                &json!(expected),
            ),
        )?;
    }
    let native_catalog = field(native, "/catalog/catalog")?
        .as_array()
        .ok_or("catalog")?;
    let headers = native_catalog
        .iter()
        .map(|entry| field(entry, "/header").cloned())
        .collect::<Result<Vec<_>>>()?;
    compare::checked(
        directory,
        "catalog",
        (&json!(headers), &json!(language.catalog)),
    )?;
    compare::checked(
        directory,
        "admissions",
        (
            field(native, "/catalog/admissions")?,
            &json!(language.admissions),
        ),
    )?;
    let mut events = field(native, "/events")?
        .as_array()
        .ok_or("events")?
        .clone();
    for event in &mut events {
        let files = event
            .get_mut("files")
            .and_then(Value::as_array_mut)
            .ok_or("files")?;
        if files.len() < prefix.len() {
            return Err("baseline files absent".into());
        }
        files.drain(..prefix.len());
    }
    std::fs::write(
        directory.join("native-events.json"),
        serde_json::to_vec(&events)?,
    )?;
    compare::checked(
        directory,
        "events",
        (&json!(events), &json!(language.events)),
    )?;
    project::compare(field(native, "/before")?, field(native, "/after_prepare")?)?;
    Ok(())
}

pub(super) fn text(
    directory: &Path,
    native: &Value,
    language: &grf::load_language_state::LanguageReport,
    pack: &Pack,
    queries: &[text_cases::Query],
) -> Result {
    let observations = field(native, "/translated")?
        .as_array()
        .ok_or("translated")?;
    if observations.len() != queries.len() {
        return Err("translation count".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for observation in observations {
        let query = queries
            .iter()
            .find(|query| observation.get("query") == Some(&json!(query.id)))
            .ok_or("unexpected translation")?;
        if !seen.insert(&query.id) {
            return Err("duplicate translation identity".into());
        }
        let point = language
            .events
            .iter()
            .find(|event| {
                event.stage == query.stage && event.file == query.file && event.line == query.line
            })
            .ok_or("Rust definition point")?;
        let map = point
            .files
            .iter()
            .find(|file| file.grfid == query.grfid)
            .and_then(|file| file.maps.get(&u32::from(query.language)));
        let translated = grf::text_translate::translate_in(
            grf::text_translate::Input {
                raw: &query.raw,
                newlines: query.newlines,
                offset: point.offset,
                context: grf::text_mapped::TextContext {
                    map,
                    genders: pack.gender_count,
                    cases: pack.case_count,
                },
            },
            &mut grf::text::Budget::new(grf::ScanLimits::default()),
            grf::string_ids::original,
        )?;
        let rust = json!({"query":query.id,"stage":query.stage,"file":query.file,"line":query.line,"bytes":translated});
        let native = json!({"query":field(observation,"/query")?,"stage":field(observation,"/stage")?,
            "file":field(observation,"/file")?,"line":field(observation,"/line")?,"bytes":field(observation,"/bytes")?});
        std::fs::write(
            directory.join(format!("text-{}.json", query.id)),
            serde_json::to_vec(&rust)?,
        )?;
        compare::checked(directory, &format!("text-{}", query.id), (&native, &rust))
            .map_err(|error| format!("query {}: {error}", query.id))?;
        project::compare(
            field(observation, "/table_before")?,
            field(observation, "/table_after")?,
        )?;
    }
    Ok(())
}
