//! Consist-specific predicates and observable rollback after earlier staged records.
pub mod rail_consist_support;
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::runtime::{
    VehicleId,
    rail::{self, ConsistError, ConsistRefusal},
};
use rail_consist_support::{Result, load, save};
use std::path::PathBuf;

fn edit(id: u32, location: (bool, &str), value: u64) -> WorldEdit {
    let (common, name) = location;
    let mut path = vec![PathElement::Field("train".into()), PathElement::Index(0)];
    if common {
        path.extend([PathElement::Field("common".into()), PathElement::Index(0)]);
    }
    path.push(PathElement::Field(name.into()));
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path,
        value: WireValue::Unsigned(value),
    }
}

#[test]
#[ignore = "requires authentic 0,27,29 fixture; corrupt inputs never go to native"]
fn late_part_refusals_roll_back_staged_records() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?);
    let original = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let directory = root.join("late-refusals");
    std::fs::create_dir(&directory)?;
    for (name, rejected, reason) in [
        (
            "up",
            edit(2, (false, "gv_flags"), 1),
            ConsistRefusal::SlopeFlags,
        ),
        (
            "down",
            edit(2, (false, "gv_flags"), 2),
            ConsistRefusal::SlopeFlags,
        ),
        (
            "both",
            edit(2, (false, "gv_flags"), 3),
            ConsistRefusal::SlopeFlags,
        ),
        (
            "motion",
            edit(2, (true, "cur_speed"), 1),
            ConsistRefusal::Motion,
        ),
        ("track", edit(2, (false, "track"), 2), ConsistRefusal::Track),
        (
            "position",
            edit(2, (true, "x_pos"), 0),
            ConsistRefusal::Geometry,
        ),
        (
            "rail-override",
            edit(2, (false, "flags"), 64),
            ConsistRefusal::Subtype,
        ),
    ] {
        let mut world = original.clone();
        world.edit_batch(vec![
            edit(0, (false, "flags"), 8),
            edit(1, (false, "flags"), 8),
            rejected,
        ])?;
        let before = world.to_savegame()?.encode(ottd_save::Compression::None)?;
        let derived = serde_json::to_value(world.derived())?;
        save(&world, &directory.join(format!("{name}.before.sav")))?;
        let result = rail::restore_consists(&mut world);
        assert!(
            matches!(result, Err(ConsistError::Part { id, reason: actual, staged_parts: 2 }) if id == VehicleId::new(2) && actual == reason),
            "{result:?}"
        );
        assert_eq!(
            world.to_savegame()?.encode(ottd_save::Compression::None)?,
            before
        );
        assert_eq!(serde_json::to_value(world.derived())?, derived);
        save(&world, &directory.join(format!("{name}.after.sav")))?;
        std::fs::write(
            directory.join(format!("{name}.result.txt")),
            format!(
                "PASS {result:?}; earlier two PoweredWagon clears were staged, none published\n"
            ),
        )?;
    }
    Ok(())
}

fn refuses(world: &mut World, reason: ConsistRefusal) -> Result {
    let before = world.to_savegame()?.encode(ottd_save::Compression::None)?;
    let derived = serde_json::to_value(world.derived())?;
    let error = rail::restore_consists(world);
    assert!(
        matches!(error, Err(ConsistError::Unsupported(actual)) if actual == reason),
        "{error:?}"
    );
    assert_eq!(
        world.to_savegame()?.encode(ottd_save::Compression::None)?,
        before
    );
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    Ok(())
}

#[test]
#[ignore = "requires authentic 0,27,29 fixture"]
fn multihead_orders_and_multiple_roots_are_specific_refusals() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?);
    let original = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let directory = root.join("specific-refusals");
    std::fs::create_dir(&directory)?;
    for (name, edits, reason) in [
        (
            "unpaired",
            vec![
                edit(0, (true, "engine_type"), 11),
                edit(0, (true, "subtype"), 41),
                edit(0, (true, "spritenum"), 8),
            ],
            ConsistRefusal::Multihead,
        ),
        (
            "two-fronts",
            vec![
                edit(0, (true, "next"), 0),
                edit(1, (true, "engine_type"), 13),
                edit(1, (true, "subtype"), 9),
                edit(1, (true, "spritenum"), 4),
            ],
            ConsistRefusal::MultipleRoots,
        ),
        (
            "orders",
            vec![edit(0, (true, "current_order.type"), 1)],
            ConsistRefusal::Orders,
        ),
        (
            "electric",
            vec![edit(0, (true, "engine_type"), 23)],
            ConsistRefusal::Engine,
        ),
    ] {
        let mut world = original.clone();
        world.edit_batch(edits)?;
        save(&world, &directory.join(format!("{name}.before.sav")))?;
        refuses(&mut world, reason)?;
        save(&world, &directory.join(format!("{name}.after.sav")))?;
        std::fs::write(
            directory.join(format!("{name}.result.txt")),
            format!("PASS typed {reason:?}; saved bytes/derived unchanged\n"),
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "requires authentic double-multihead fixture"]
fn rejects_native_multihead_repair_before_publication() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?);
    let mut world = load(&root.join("prepare/save/autosave/exit.sav"))?;
    world.edit_batch(vec![
        edit(1, (true, "subtype"), 40),
        edit(1, (true, "spritenum"), 8),
    ])?;
    save(&world, &root.join("repair.before.sav"))?;
    refuses(&mut world, ConsistRefusal::Multihead)?;
    save(&world, &root.join("repair.after.sav"))?;
    std::fs::write(
        root.join("repair.result.txt"),
        "PASS native pair repair refused with saved bytes/derived unchanged; native not called\n",
    )?;
    Ok(())
}
