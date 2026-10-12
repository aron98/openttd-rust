//! Synthetic source-predicate controls; these are not native parity fixtures.
use super::{State, facilities as score};
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use std::path::PathBuf;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires genuine SERVICE_INPUT and SERVICE_OUTPUT; synthetic station predicates"]
fn service_facility_owner_boundaries_and_omitted_vehicle_guard() -> Result {
    let bytes = std::fs::read(std::env::var("SERVICE_INPUT")?)?;
    let output = PathBuf::from(std::env::var("SERVICE_OUTPUT")?);
    let mut results = Vec::new();
    for (name, owner, facilities, load, unload, copies, expected) in [
        ("inactive", 0, 1, 21, 21, 1, 0),
        ("load-boundary", 0, 1, 20, 21, 1, 1),
        ("unload-boundary", 0, 1, 21, 20, 1, 1),
        ("both-active", 0, 1, 20, 20, 1, 1),
        ("different-owner", 1, 1, 0, 0, 1, 0),
        ("waypoint", 0, 129, 0, 0, 1, 0),
        ("unused-sign", 0, 0, 0, 0, 1, 0),
        ("multiple-facilities", 0, 31, 0, 0, 1, 5),
        ("facility-clamp", 0, 31, 0, 0, 17, 85),
    ] {
        let mut world = World::decode(&Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
        let before = world.saved_json()?;
        {
            let industries = crate::runtime::IndustryCaches::restore(&world)?;
            let mut state = State::load(&mut world, None, industries);
            for station in [0, 1] {
                state.apply(station_field(station, "time_since_load", 21))?;
                state.apply(station_field(station, "time_since_unload", 21))?;
            }
            state.apply(station_field(0, "time_since_load", load))?;
            state.apply(station_field(0, "time_since_unload", unload))?;
            state.apply(WorldEdit::Field {
                chunk: *b"STNN",
                record: 0,
                path: vec![PathElement::Field("facilities".into())],
                value: WireValue::Unsigned(facilities),
            })?;
            state.apply(WorldEdit::Field {
                chunk: *b"STNN",
                record: 0,
                path: vec![
                    PathElement::Field("normal".into()),
                    PathElement::Index(0),
                    PathElement::Field("base".into()),
                    PathElement::Index(0),
                    PathElement::Field("owner".into()),
                ],
                value: WireValue::Unsigned(owner),
            })?;
            let copy = state.table(b"STNN")?.record(0).ok_or("station")?.clone();
            for index in 1..copies {
                state.apply(WorldEdit::InsertRecord {
                    chunk: *b"STNN",
                    record: index + 1,
                    value: copy.clone(),
                })?;
            }
            assert_eq!(score(&state, 0)?, expected, "{name}");
        }
        assert_eq!(world.saved_json()?, before);
        results.push(serde_json::json!({"case":name,"synthetic":true,"score":expected,"world_unchanged":true}));
    }
    vehicle_guards(&bytes, &mut results)?;
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&results)?,
    )?;
    Ok(())
}
fn station_field(id: u32, name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"STNN",
        record: id,
        path: vec![
            PathElement::Field("normal".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value: WireValue::Unsigned(value),
    }
}
fn vehicle_guards(bytes: &[u8], results: &mut Vec<serde_json::Value>) -> Result {
    for (age, profit, owner, expected) in [
        (0, 1, 0, (1, 0)),
        (731, 1, 0, (1, 0)),
        (731, 0, 0, (0, 0)),
        (731, -1, 0, (0, 0)),
        (0, 1, 1, (0, 0)),
    ] {
        let mut world = World::decode(&Savegame::decode(bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
        let industries = crate::runtime::IndustryCaches::restore(&world)?;
        let mut state = State::load(&mut world, None, industries);
        for (field, value) in [
            ("economy_age", WireValue::Unsigned(age)),
            ("profit_last_year", WireValue::Signed(profit)),
            ("owner", WireValue::Unsigned(owner)),
        ] {
            state.apply(WorldEdit::Field {
                chunk: *b"VEHS",
                record: 0,
                path: vec![
                    PathElement::Field("roadveh".into()),
                    PathElement::Index(0),
                    PathElement::Field("common".into()),
                    PathElement::Index(0),
                    PathElement::Field(field.into()),
                ],
                value,
            })?;
        }
        let result = crate::world_ticks::company_vehicle_rating::vehicles(&state, 0)?;
        assert_eq!(result, expected);
        results.push(serde_json::json!({"synthetic":true,"age":age,"profit_last_year":profit,"owner":owner,"raw_vehicle_parts":result}));
    }
    Ok(())
}
