use super::*;

#[test]
#[ignore = "SALE_SOURCE and SALE_SETUP: position original fleet for original sale cleanup"]
fn prepare_sale_cleanup() -> Result {
    let output = PathBuf::from(std::env::var("SALE_SETUP")?);
    std::fs::create_dir(&output)?;
    let mut world = read(Path::new(&std::env::var("SALE_SOURCE")?))?;
    let tile = u32::try_from(
        world
            .map()
            .tiles()
            .iter()
            .position(|t| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
            .ok_or("depot")?,
    )?;
    let ids: Vec<_> = world
        .tables()
        .get(b"VEHS")
        .ok_or("VEHS")?
        .records()
        .keys()
        .copied()
        .collect();
    let runtime = SimulationRuntime::restore_vanilla(world.clone())?;
    let engine = runtime
        .vehicle(VehicleId::new(*ids.first().ok_or("fleet")?))?
        .engine_id()?;
    let width = world.map().width();
    let mut height = 0;
    for offset in [0, 1, width, width.saturating_add(1)] {
        height = height.max(
            world
                .map()
                .tiles()
                .get(usize::try_from(tile.saturating_add(offset))?)
                .ok_or("corner")?
                .height(),
        );
    }
    let mut edits = vec![field(
        *b"PATS",
        "construction.command_pause_level",
        WireValue::Unsigned(2),
    )];
    for id in &ids {
        for (name, value) in [
            ("tile", WireValue::Unsigned(u64::from(tile))),
            (
                "x_pos",
                WireValue::Unsigned(u64::from(
                    (tile % width).saturating_mul(16).saturating_add(8),
                )),
            ),
            (
                "y_pos",
                WireValue::Unsigned(u64::from(
                    (tile / width).saturating_mul(16).saturating_add(8),
                )),
            ),
            ("z_pos", WireValue::Signed(i64::from(height) * 8)),
            ("cur_speed", WireValue::Unsigned(0)),
            ("vehstatus", WireValue::Unsigned(11)),
        ] {
            edits.push(vehicle(*id, name, value));
        }
        edits.push(WorldEdit::Field {
            chunk: *b"VEHS",
            record: *id,
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("state".into()),
            ],
            value: WireValue::Unsigned(254),
        });
        edits.push(vehicle(
            *id,
            "cargo.action_counts",
            carried_actions(&world, *id)?,
        ));
    }
    world.edit_batch(edits)?;
    write(&world, &output, "cleanup")?;
    std::fs::write(
        output.join("fleet.json"),
        serde_json::to_vec(&serde_json::json!({"ids":ids,"tile":tile,"engine":engine}))?,
    )?;
    Ok(())
}

fn carried_actions(world: &World, id: u32) -> Result<WireValue> {
    let saved = world.saved_json()?;
    let actions = saved
        .pointer(&format!(
            "/chunks/VEHS/records/{id}/roadveh/0/common/0/cargo.action_counts"
        ))
        .and_then(serde_json::Value::as_array)
        .ok_or("cargo actions")?;
    let count = actions
        .iter()
        .try_fold(0_u64, |total, value| -> Result<u64> {
            Ok(total
                .checked_add(value.as_u64().ok_or("cargo count")?)
                .ok_or("cargo overflow")?)
        })?;
    Ok(WireValue::Array(vec![
        WireValue::Unsigned(0),
        WireValue::Unsigned(0),
        WireValue::Unsigned(count),
        WireValue::Unsigned(0),
    ]))
}
