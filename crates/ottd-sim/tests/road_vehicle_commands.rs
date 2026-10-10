//! Service interval commands publish saved edits without replacing road caches.
use ottd_save::{
    Savegame, TableRecord, TableSchema, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{Command, CommandMode, CommandRequest, runtime::SimulationRuntime};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn field<'a>(schema: &TableSchema, row: &'a TableRecord, name: &str) -> Result<&'a WireValue> {
    schema
        .fields()
        .iter()
        .zip(row.values())
        .find_map(|(f, v)| (f.name() == name).then_some(v))
        .ok_or_else(|| name.to_owned().into())
}
fn base() -> Result<World> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let table = world.tables().get(b"VEHS").ok_or("VEHS")?;
    let edits = table
        .records()
        .iter()
        .filter_map(|(id, row)| match field(table.schema(), row, "type") {
            Ok(WireValue::Unsigned(1)) => None,
            _ => Some(WorldEdit::RemoveRecord {
                chunk: *b"VEHS",
                record: *id,
            }),
        })
        .collect::<Vec<_>>();
    world.edit_batch(edits)?;
    world.edit_batch(vec![
        common_edit("owner", 0),
        common_edit("vehicle_flags", 0),
    ])?;
    world.edit_field(
        *b"DATE",
        0,
        &[PathElement::Field("pause_mode".into())],
        WireValue::Unsigned(0),
    )?;
    Ok(world)
}
const fn request(interval: u16, custom: bool, percent: bool) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::ChangeServiceInterval {
            vehicle: 21,
            interval,
            custom,
            percent,
        },
    }
}
fn common_edit(name: &str, value: u64) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: 21,
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
#[test]
fn service_interval_commits_only_requested_fields_and_preserves_cache_identity() -> Result {
    // Given a native-created road front with unrelated flags already set.
    let mut world = base()?;
    world.edit_batch(vec![common_edit("vehicle_flags", 0x85)])?;
    let mut expected = world.clone();
    expected.edit_batch(vec![
        common_edit("service_interval", 42),
        common_edit("vehicle_flags", 0x385),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let caches = runtime.road_caches().clone();
    let cache_pointer = std::ptr::from_ref(
        runtime
            .road_caches()
            .get(&ottd_sim::runtime::VehicleId::new(21))
            .ok_or("road")?,
    );
    // When a custom percent interval is posted through the owned runtime.
    let receipt = runtime.execute_command(&request(42, true, true))?;
    // Then complete saved state (including RNG) and cache identity are exact.
    assert!(receipt.posted);
    assert_eq!(receipt.result.ok_or("result")?.cost, 0);
    assert_eq!(runtime.world().saved_json()?, expected.saved_json()?);
    assert_eq!(
        serde_json::to_value(runtime.world().derived())?,
        serde_json::to_value(expected.derived())?
    );
    assert_eq!(runtime.road_caches(), &caches);
    assert!(std::ptr::eq(
        runtime
            .road_caches()
            .get(&ottd_sim::runtime::VehicleId::new(21))
            .ok_or("road")?,
        cache_pointer
    ));
    Ok(())
}
#[test]
fn rejected_interval_preserves_world_rng_and_caches() -> Result {
    // Given an admitted native road vehicle.
    let mut runtime = SimulationRuntime::restore_vanilla(base()?)?;
    let before = runtime.world().saved_json()?;
    let derived = serde_json::to_value(runtime.world().derived())?;
    let caches = runtime.road_caches().clone();
    // When percent interval is outside native bounds.
    let receipt = runtime.execute_command(&request(91, true, true))?;
    // Then native rejection publishes nothing.
    assert!(!receipt.posted);
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("CMD_ERROR")
    );
    assert_eq!(runtime.world().saved_json()?, before);
    assert_eq!(serde_json::to_value(runtime.world().derived())?, derived);
    assert_eq!(runtime.road_caches(), &caches);
    Ok(())
}

fn setting(chunk: [u8; 4], name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
#[test]
fn custom_bounds_follow_percent_calendar_and_wallclock_units() -> Result {
    let base = base()?;
    for (wallclock, percent, lower, upper) in [
        (false, true, 5, 90),
        (false, false, 30, 800),
        (true, false, 1, 30),
    ] {
        for interval in [lower - 1, lower, upper, upper + 1] {
            let mut world = base.clone();
            world.edit_batch(vec![setting(
                *b"PATS",
                "economy.timekeeping_units",
                WireValue::Unsigned(u64::from(wallclock)),
            )])?;
            let mut runtime = SimulationRuntime::restore_vanilla(world)?;
            let before = runtime.world().saved_json()?;
            let receipt = runtime.execute_command(&request(interval, true, percent))?;
            assert_eq!(receipt.posted, (lower..=upper).contains(&interval));
            if !receipt.posted {
                assert_eq!(runtime.world().saved_json()?, before);
            }
        }
    }
    Ok(())
}
#[test]
fn company_defaults_override_both_requested_values_without_clamping() -> Result {
    let mut world = base()?;
    let company_setting = |name: &str, value| WorldEdit::Field {
        chunk: *b"PLYR",
        record: 0,
        path: vec![
            PathElement::Field("settings".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    };
    world.edit_batch(vec![
        company_setting("settings.vehicle.servint_roadveh", WireValue::Unsigned(0)),
        company_setting("settings.vehicle.servint_ispercent", WireValue::Signed(1)),
        common_edit("vehicle_flags", 0x185),
    ])?;
    let mut expected = world.clone();
    expected.edit_batch(vec![
        common_edit("service_interval", 0),
        common_edit("vehicle_flags", 0x285),
    ])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let receipt = runtime.execute_command(&request(65535, false, false))?;
    assert!(receipt.posted);
    assert_eq!(runtime.world().saved_json()?, expected.saved_json()?);
    Ok(())
}
#[test]
fn estimate_and_missing_id_preserve_committed_state() -> Result {
    for missing in [false, true] {
        let mut runtime = SimulationRuntime::restore_vanilla(base()?)?;
        let before = runtime.world().saved_json()?;
        let caches = runtime.road_caches().clone();
        let mut command = request(42, true, true);
        if missing {
            command.command = Command::ChangeServiceInterval {
                vehicle: u32::MAX,
                interval: 42,
                custom: true,
                percent: true,
            };
        } else {
            command.mode = CommandMode::Estimate;
        }
        let receipt = runtime.execute_command(&command)?;
        assert_eq!(receipt.posted, !missing);
        assert!(receipt.exec.is_none());
        assert_eq!(runtime.world().saved_json()?, before);
        assert_eq!(runtime.road_caches(), &caches);
    }
    Ok(())
}
#[test]
fn runtime_rejects_unimplemented_cache_mutations_before_changing_world() -> Result {
    let mut runtime = SimulationRuntime::restore_vanilla(base()?)?;
    let before = runtime.world().saved_json()?;
    let mut command = request(42, true, true);
    command.command = Command::RenameCompany {
        text: "new name".into(),
    };
    assert!(matches!(
        runtime.execute_command(&command),
        Err(ottd_sim::CommandError::Unsupported(_))
    ));
    assert_eq!(runtime.world().saved_json()?, before);
    Ok(())
}
#[test]
fn pause_gate_uses_vehicle_management_level_and_marks_successful_paused_commands() -> Result {
    for level in [0, 1] {
        let mut world = base()?;
        world.edit_batch(vec![
            setting(*b"DATE", "pause_mode", WireValue::Unsigned(1)),
            setting(
                *b"PATS",
                "construction.command_pause_level",
                WireValue::Unsigned(level),
            ),
        ])?;
        let mut expected = world.clone();
        if level == 1 {
            expected.edit_batch(vec![
                common_edit("service_interval", 42),
                common_edit("vehicle_flags", 0x300),
                setting(*b"DATE", "pause_mode", WireValue::Unsigned(129)),
            ])?;
        }
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        let receipt = runtime.execute_command(&request(42, true, true))?;
        assert_eq!(receipt.posted, level == 1);
        assert_eq!(runtime.world().saved_json()?, expected.saved_json()?);
    }
    Ok(())
}

#[test]
fn ownership_rejection_precedes_custom_interval_validation() -> Result {
    let mut world = base()?;
    world.edit_batch(vec![common_edit("owner", 1)])?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = runtime.world().saved_json()?;
    let caches = runtime.road_caches().clone();
    let receipt = runtime.execute_command(&request(0, true, true))?;
    let cost = receipt.result.ok_or("result")?;
    assert_eq!(cost.error.as_deref(), Some("STR_ERROR_OWNED_BY"));
    assert_eq!(cost.error_params, vec![0x881D, 1]);
    assert_eq!(runtime.world().saved_json()?, before);
    assert_eq!(runtime.road_caches(), &caches);
    Ok(())
}
#[test]
fn nonprimary_road_vehicle_is_native_rejection_on_world_path() -> Result {
    let mut world = base()?;
    world.edit_batch(vec![common_edit("subtype", 0)])?;
    let before = world.saved_json()?;
    let receipt = ottd_sim::execute_command(&mut world, &request(42, true, true))?;
    assert_eq!(
        receipt.result.ok_or("result")?.error.as_deref(),
        Some("CMD_ERROR")
    );
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
#[test]
fn other_vehicle_family_is_explicitly_unsupported() -> Result {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let table = world.tables().get(b"VEHS").ok_or("VEHS")?;
    let id = *table
        .records()
        .iter()
        .find(|(_, row)| {
            matches!(
                field(table.schema(), row, "type"),
                Ok(WireValue::Unsigned(0))
            )
        })
        .ok_or("train")?
        .0;
    let before = world.saved_json()?;
    let mut command = request(42, true, true);
    command.mode = CommandMode::Estimate;
    command.command = Command::ChangeServiceInterval {
        vehicle: id,
        interval: 42,
        custom: true,
        percent: true,
    };
    assert!(matches!(
        ottd_sim::execute_command(&mut world, &command),
        Err(ottd_sim::CommandError::Unsupported(_))
    ));
    assert_eq!(world.saved_json()?, before);
    Ok(())
}
