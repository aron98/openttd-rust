//! Safe saved controls and Rust-only invalid graph rollback for consist design.
pub mod rail_consist_support;
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::runtime::rail::{self, RailRefusal, RailRestoreError};
use rail_consist_support::{Result, load, native, observation, save, world_equal};
use std::path::PathBuf;

fn field(id: u32, location: (bool, &str), value: u64) -> WorldEdit {
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

fn encoded(world: &World) -> Result<Vec<u8>> {
    Ok(world.to_savegame()?.encode(ottd_save::Compression::None)?)
}

#[test]
#[ignore = "requires authentic 0,27,29 consist; never sends corrupt chains to native"]
fn rust_only_invalid_chain_controls() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?);
    let original = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let directory = root.join("rust-only-refusals");
    std::fs::create_dir(&directory)?;
    for (name, edit) in [
        ("self-cycle", field(0, (true, "next"), 1)),
        ("dangling", field(0, (true, "next"), 99999)),
        ("duplicate-predecessor", field(2, (true, "next"), 2)),
        ("detached-wagons", field(0, (true, "next"), 0)),
        ("articulated-bit", field(1, (true, "subtype"), 2)),
        ("free-wagon", field(0, (true, "subtype"), 20)),
        ("unpaired-multihead", field(0, (true, "subtype"), 41)),
        ("invalid-owner", field(1, (true, "owner"), 15)),
        ("invalid-cargo", field(1, (true, "cargo_type"), 255)),
        ("saved-going-up", field(1, (false, "gv_flags"), 1)),
        ("saved-going-down", field(1, (false, "gv_flags"), 2)),
        ("saved-both-slopes", field(1, (false, "gv_flags"), 3)),
    ] {
        let mut world = original.clone();
        let before_edit = encoded(&world)?;
        let before_derived = serde_json::to_value(world.derived())?;
        std::fs::write(
            directory.join(format!("{name}.edit-before.sav")),
            &before_edit,
        )?;
        std::fs::write(
            directory.join(format!("{name}.derived-before.json")),
            serde_json::to_vec(&before_derived)?,
        )?;
        let result = if let Err(error) = world.edit_batch(vec![edit]) {
            assert_eq!(encoded(&world)?, before_edit);
            assert_eq!(serde_json::to_value(world.derived())?, before_derived);
            format!("World edit refusal (before rail): {error}")
        } else {
            let before = encoded(&world)?;
            let derived = serde_json::to_value(world.derived())?;
            std::fs::write(directory.join(format!("{name}.before.sav")), &before)?;
            let result = rail::restore(&mut world);
            assert!(matches!(
                result,
                Err(RailRestoreError::Unsupported(RailRefusal::VehicleRoster))
            ));
            assert_eq!(encoded(&world)?, before);
            assert_eq!(serde_json::to_value(world.derived())?, derived);
            format!(
                "Current generic roster refusal, NOT a future consist-specific guard: {result:?}"
            )
        };
        save(&world, &directory.join(format!("{name}.after.sav")))?;
        std::fs::write(
            directory.join(format!("{name}.derived-after.json")),
            serde_json::to_vec(&world.derived())?,
        )?;
        std::fs::write(directory.join(format!("{name}.result.txt")), result)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Control {
    Acceleration,
    Powered,
    Capacity,
    Coupled,
}

fn saved_control(control: Control, name: &str) -> Result {
    let root = PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?);
    let baseline = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let directory = root.join(name);
    std::fs::create_dir(&directory)?;
    let mut input = baseline.clone();
    let mut expected_saved = baseline;
    let edits = match control {
        Control::Acceleration => vec![field(0, (true, "acceleration"), 255)],
        Control::Powered => vec![
            field(0, (false, "flags"), 8),
            field(1, (false, "flags"), 8),
            field(2, (false, "flags"), 8),
        ],
        Control::Capacity => vec![field(1, (true, "cargo_cap"), 37)],
        Control::Coupled => vec![
            field(0, (true, "acceleration"), 255),
            field(1, (false, "flags"), 8),
            field(1, (true, "cargo_cap"), 37),
        ],
    };
    if matches!(control, Control::Capacity | Control::Coupled) {
        expected_saved.edit_batch(vec![field(1, (true, "cargo_cap"), 37)])?;
    }
    input.edit_batch(edits)?;
    let path = directory.join("input.sav");
    save(&input, &path)?;
    native(&directory.join("observe"), &path, false)?;
    let after = load(&directory.join("observe/save/autosave/exit.sav"))?;
    world_equal(
        &after,
        &expected_saved,
        &directory.join("native-saved-control"),
    )?;
    let serialized = directory.join("native-after.rust.sav");
    save(&after, &serialized)?;
    native(&directory.join("continuation"), &serialized, false)?;
    assert_eq!(
        observation(&directory.join("observe"))?,
        observation(&directory.join("continuation"))?
    );
    world_equal(
        &load(&directory.join("continuation/save/autosave/exit.sav"))?,
        &after,
        &directory.join("continuation-world"),
    )?;
    let stderr = std::fs::read_to_string(directory.join("observe/stderr.log"))?;
    assert!(!stderr.contains("[grf:0]"));
    std::fs::write(
        directory.join("native-control.txt"),
        "PASS original after-load complete saved mutation set and full-cache/RNG/save continuation; capacity stays saved, no vanilla GRF diagnostic\n",
    )?;
    let before = encoded(&input)?;
    let result = rail::restore(&mut input);
    std::fs::write(directory.join("rust-red.txt"), format!("{result:?}\n"))?;
    assert_eq!(encoded(&input)?, before);
    assert!(
        result.is_ok(),
        "missing consist restoration boundary: {result:?}"
    );
    Ok(())
}

#[test]
#[ignore = "intentional native-backed stale acceleration RED"]
fn stale_acceleration() -> Result {
    saved_control(Control::Acceleration, "stale-acceleration")
}
#[test]
#[ignore = "intentional native-backed powered flags RED"]
fn powered_flags() -> Result {
    saved_control(Control::Powered, "powered-flags")
}
#[test]
#[ignore = "intentional native-backed saved capacity RED"]
fn saved_capacity() -> Result {
    saved_control(Control::Capacity, "saved-capacity")
}
#[test]
#[ignore = "intentional native-backed coupled saved mutation RED"]
fn coupled_saved_mutations() -> Result {
    saved_control(Control::Coupled, "coupled")
}
