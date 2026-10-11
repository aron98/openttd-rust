//! Saved-field controls at the bounded original train after-load boundary.
pub mod rail_restoration_support;
use ottd_save::{
    Compression, TableRecord, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::runtime::rail::{self, RailRefusal, RailRestoreError};
use rail_restoration_support::{Result, load, native, verify};
use std::path::PathBuf;

fn field(common: bool, name: &str, value: u64) -> WorldEdit {
    let mut path = vec![PathElement::Field("train".into()), PathElement::Index(0)];
    if common {
        path.extend([PathElement::Field("common".into()), PathElement::Index(0)]);
    }
    path.push(PathElement::Field(name.into()));
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: 0,
        path,
        value: WireValue::Unsigned(value),
    }
}

#[test]
#[ignore = "requires original engine-0 fixture, OTTD_RAIL_EVIDENCE and OTTD_RAIL_ORACLE"]
fn native_saved_mutations_and_silent_capacity_mismatch() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_RAIL_EVIDENCE")?);
    let original = load(&root.join("prepare/save/autosave/exit.sav"))?;
    for (name, edits) in [
        ("stale-acceleration", vec![field(true, "acceleration", 0)]),
        ("powered-wagon", vec![field(false, "flags", 8)]),
        ("saved-capacity", vec![field(true, "cargo_cap", 37)]),
        (
            "coupled",
            vec![
                field(true, "acceleration", 255),
                field(false, "flags", 8),
                field(true, "cargo_cap", 37),
            ],
        ),
    ] {
        // Given explicit saved-value corruption controls on an authentic original fixture.
        let directory = root.join(name);
        std::fs::create_dir(&directory)?;
        let mut world = original.clone();
        world.edit_batch(edits)?;
        let input = directory.join("input.sav");
        std::fs::write(&input, world.to_savegame()?.encode(Compression::None)?)?;
        native(&directory.join("observe"), &input, false)?;

        // When Rust and original independently restore, save and reload that state.
        verify(&directory, &input)?;

        // Then vanilla capacity mismatch is silent, with complete saved/cache equality.
        let stderr = std::fs::read_to_string(directory.join("observe/stderr.log"))?;
        assert!(!stderr.contains("[grf:0]"));
    }
    Ok(())
}

#[test]
#[ignore = "requires original rail fixture and OTTD_RAIL_ORACLE"]
fn passive_observer_does_not_change_saved_bytes() -> Result {
    // Given the same authentic paused native save used by the enabled observer.
    let root = PathBuf::from(std::env::var("OTTD_RAIL_EVIDENCE")?);
    let directory = root.join("observer-disabled");
    let input = root.join("prepare/save/autosave/exit.sav");

    // When the same native executable loads with the passive observer disabled.
    native(&directory, &input, true)?;

    // Then the complete native serialized save remains byte-identical.
    let disabled = std::fs::read(directory.join("save/autosave/exit.sav"))?;
    let enabled = std::fs::read(root.join("observe/save/autosave/exit.sav"))?;
    assert_eq!(disabled, enabled);
    std::fs::write(
        directory.join("neutrality.txt"),
        "PASS exact native save bytes with observer enabled/disabled\n",
    )?;
    Ok(())
}

fn unchanged_refusal(mut world: World, reason: RailRefusal, directory: &std::path::Path) -> Result {
    let before = world.to_savegame()?.encode(Compression::None)?;
    let derived_before = serde_json::to_value(world.derived())?;
    std::fs::write(directory.with_extension("before.sav"), &before)?;

    let result = rail::restore(&mut world);

    assert!(
        matches!(&result, Err(RailRestoreError::Unsupported(actual)) if *actual == reason),
        "{result:?}"
    );
    let after = world.to_savegame()?.encode(Compression::None)?;
    assert_eq!(before, after);
    assert_eq!(derived_before, serde_json::to_value(world.derived())?);
    std::fs::write(directory.with_extension("after.sav"), after)?;
    std::fs::write(
        directory.with_extension("result.txt"),
        format!("PASS {result:?}; whole saved bytes and structural state unchanged\n"),
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original engine-0 fixture and OTTD_RAIL_EVIDENCE"]
fn unsupported_contexts_refuse_without_saved_publication() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_RAIL_EVIDENCE")?);
    let original = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let directory = root.join("refusals");
    std::fs::create_dir(&directory)?;
    for (name, edit, reason) in [
        (
            "multihead",
            field(true, "subtype", 41),
            RailRefusal::Consist,
        ),
        (
            "articulated",
            field(true, "subtype", 11),
            RailRefusal::Consist,
        ),
        (
            "free-wagon",
            field(true, "subtype", 20),
            RailRefusal::Consist,
        ),
        (
            "active-order",
            field(true, "current_order.type", 1),
            RailRefusal::Orders,
        ),
        ("moving", field(true, "cur_speed", 1), RailRefusal::Motion),
        ("started", field(true, "vehstatus", 8), RailRefusal::Motion),
        (
            "depot-context",
            field(true, "tile", 520),
            RailRefusal::Track,
        ),
        (
            "clear-context",
            field(true, "tile", 600),
            RailRefusal::Track,
        ),
        ("wrong-track", field(false, "track", 2), RailRefusal::Track),
        (
            "wrong-direction",
            field(true, "direction", 3),
            RailRefusal::Track,
        ),
        (
            "wagon-engine",
            field(true, "engine_type", 27),
            RailRefusal::Engine,
        ),
        (
            "multihead-engine",
            field(true, "engine_type", 11),
            RailRefusal::Engine,
        ),
    ] {
        // Given a structurally valid but unsupported saved context and stale acceleration.
        let mut world = original.clone();
        world.edit_batch(vec![field(true, "acceleration", 255), edit])?;

        // When restoration refuses, then all saved and structural state remains unchanged.
        unchanged_refusal(world, reason, &directory.join(name))?;
    }
    let mut empty = original;
    empty.edit_batch(vec![WorldEdit::RemoveRecord {
        chunk: *b"VEHS",
        record: 0,
    }])?;
    unchanged_refusal(
        empty,
        RailRefusal::VehicleRoster,
        &directory.join("empty-roster"),
    )?;
    Ok(())
}

#[test]
#[ignore = "requires original rail fixture and OTTD_RAIL_EVIDENCE"]
fn populated_cargo_refuses_without_saved_publication() -> Result {
    // Given a structurally validated, explicitly synthetic single-packet saved control.
    let root = PathBuf::from(std::env::var("OTTD_RAIL_EVIDENCE")?);
    let mut world = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let values = world
        .tables()
        .get(b"CAPA")
        .ok_or("cargo table")?
        .schema()
        .fields()
        .iter()
        .map(|field| match field.name() {
            "source" | "source_id" => Ok(WireValue::Unsigned(65535)),
            "source_xy" | "loaded_at_xy" => Ok(WireValue::Unsigned(519)),
            "count" => Ok(WireValue::Unsigned(1)),
            "periods_in_transit" | "source_type" => Ok(WireValue::Unsigned(0)),
            "feeder_share" | "travelled.x" | "travelled.y" => Ok(WireValue::Signed(0)),
            _ => Err("unexpected cargo schema"),
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let path = |name: &str| {
        vec![
            PathElement::Field("train".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ]
    };
    world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: 0,
            value: TableRecord::new(values),
        },
        WorldEdit::ArrayList {
            chunk: *b"VEHS",
            record: 0,
            path: path("cargo.packets"),
            values: vec![WireValue::Unsigned(1)],
        },
        WorldEdit::Field {
            chunk: *b"VEHS",
            record: 0,
            path: path("cargo.action_counts"),
            value: WireValue::Array(vec![
                WireValue::Unsigned(1),
                WireValue::Unsigned(0),
                WireValue::Unsigned(0),
                WireValue::Unsigned(0),
            ]),
        },
        field(true, "acceleration", 255),
    ])?;

    // When only Rust receives this unsupported cargo domain, then no saved state publishes.
    unchanged_refusal(world, RailRefusal::Cargo, &root.join("populated-cargo"))?;
    Ok(())
}
