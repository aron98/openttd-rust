//! Typed test-only setup over a freshly original-created road fleet.
use ottd_save::{
    Compression, Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use std::path::PathBuf;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn field(chunk: [u8; 4], record: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
fn vehicle(id: u32, name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value: WireValue::Unsigned(value),
    }
}
fn company(name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"PLYR",
        record: 0,
        path: vec![
            PathElement::Field("settings".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    }
}
#[test]
#[ignore = "fresh native service fixture setup: SERVICE_SOURCE and SERVICE_INPUT_DIR"]
fn prepare_service_inputs() -> Result {
    let source = PathBuf::from(std::env::var("SERVICE_SOURCE")?);
    let output = PathBuf::from(std::env::var("SERVICE_INPUT_DIR")?);
    std::fs::create_dir(&output)?;
    let base = World::decode(&Savegame::decode(
        &std::fs::read(&source)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let id = *base
        .tables()
        .get(b"VEHS")
        .and_then(|t| t.records().keys().next())
        .ok_or("road fleet")?;
    let runtime = ottd_sim::runtime::SimulationRuntime::restore_vanilla(base.clone())?;
    assert!(runtime.road_caches().len() >= 6);
    let company_row = base
        .tables()
        .get(b"PLYR")
        .and_then(|t| t.records().get(&0))
        .ok_or("company0")?
        .clone();
    for (name, wallclock, percent, pause_level, subtype, other_owner) in [
        ("percent", false, true, 1, 1, false),
        ("calendar", false, false, 1, 1, false),
        ("wallclock", true, false, 1, 1, false),
        ("defaults-percent", false, true, 1, 1, false),
        ("defaults-days", false, false, 1, 1, false),
        ("pause-gate", false, true, 0, 1, false),
        ("missing", false, true, 1, 1, false),
        ("nonprimary", false, true, 1, 0, false),
        ("nonowner", false, true, 1, 1, true),
        ("resume", false, true, 1, 1, false),
    ] {
        let mut world = base.clone();
        let mut edits = vec![
            field(
                *b"PATS",
                0,
                "economy.timekeeping_units",
                WireValue::Unsigned(u64::from(wallclock)),
            ),
            field(
                *b"PATS",
                0,
                "construction.command_pause_level",
                WireValue::Unsigned(pause_level),
            ),
            field(*b"DATE", 0, "pause_mode", WireValue::Unsigned(1)),
            vehicle(id, "subtype", subtype),
            vehicle(id, "vehicle_flags", 0x385),
            company(
                "settings.vehicle.servint_ispercent",
                WireValue::Signed(i64::from(percent)),
            ),
            company("settings.vehicle.servint_roadveh", WireValue::Unsigned(0)),
        ];
        if other_owner {
            edits.push(WorldEdit::InsertRecord {
                chunk: *b"PLYR",
                record: 1,
                value: company_row.clone(),
            });
            edits.push(vehicle(id, "owner", 1));
        }
        world.edit_batch(edits)?;
        std::fs::write(
            output.join(format!("{name}.sav")),
            world.to_savegame()?.encode(Compression::None)?,
        )?;
        std::fs::write(
            output.join(format!("{name}.world.json")),
            serde_json::to_vec(&world.saved_json()?)?,
        )?;
        std::fs::write(
            output.join(format!("{name}.derived.json")),
            serde_json::to_vec(world.derived())?,
        )?;
    }
    std::fs::write(
        output.join("fleet.json"),
        serde_json::to_vec(
            &serde_json::json!({"vehicle":id,"count":runtime.road_caches().len(),"source":source}),
        )?,
    )?;
    Ok(())
}
