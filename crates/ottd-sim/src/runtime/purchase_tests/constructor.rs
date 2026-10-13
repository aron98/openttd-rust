use super::*;

#[test]
fn purchase_retains_native_serialization_global() -> Result {
    let (mut runtime, tile) = fixture()?;
    assert!(runtime.execute_command(&request(tile))?.posted);
    let mut world = runtime.into_world();
    let edits = world
        .tables()
        .get(b"VEHS")
        .ok_or("VEHS")?
        .records()
        .keys()
        .map(|id| WorldEdit::Field {
            chunk: *b"VEHS",
            record: *id,
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("common".into()),
                PathElement::Index(0),
                PathElement::Field("cargo_paid_for".into()),
            ],
            value: WireValue::Unsigned(37),
        })
        .collect();
    world.edit_batch(edits)?;
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let receipt = runtime.execute_command(&request(tile))?;
    let Some(crate::CommandReturn::Vehicle { vehicle, .. }) =
        receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle result".into());
    };
    assert_eq!(
        runtime.world.saved_json()?.pointer(&format!(
            "/chunks/VEHS/records/{vehicle}/roadveh/0/common/0/cargo_paid_for"
        )),
        Some(&serde_json::json!(37))
    );
    Ok(())
}

#[test]
fn depot_direction_height_and_dynamic_engine_fields_drive_new_saved_state() -> Result {
    for direction in 0..4 {
        let (mut runtime, tile) = fixture()?;
        let tile_index = usize::try_from(tile)?;
        let mut raw =
            TileRawParts::from(runtime.world.map().tiles().get(tile_index).ok_or("tile")?);
        raw.m5 = (raw.m5 & !3) | direction;
        runtime.world.edit_batch(vec![
            WorldEdit::Tile {
                index: tile,
                value: raw.into(),
            },
            crate::world_access::field_edit(
                *b"ENGN",
                116,
                "reliability",
                WireValue::Unsigned(1234),
            ),
            crate::world_access::field_edit(
                *b"ENGN",
                116,
                "reliability_spd_dec",
                WireValue::Unsigned(17),
            ),
            crate::world_access::field_edit(*b"ENGN", 116, "flags", WireValue::Unsigned(2)),
            crate::world_access::field_edit(
                *b"PATS",
                0,
                "vehicle.extend_vehicle_life",
                WireValue::Unsigned(5),
            ),
        ])?;
        let calendar = crate::world_access::signed(&runtime.world, b"DATE", 0, "date")?;
        let economy = crate::world_access::signed(&runtime.world, b"DATE", 0, "economy_date")?;
        let receipt = runtime.execute_command(&request(tile))?;
        assert!(receipt.posted);
        let json = runtime.world.saved_json()?;
        let common = json
            .pointer("/chunks/VEHS/records/0/roadveh/0/common/0")
            .ok_or("common")?;
        assert_eq!(
            common.get("direction"),
            Some(&serde_json::json!(
                direction.saturating_mul(2).saturating_add(1)
            ))
        );
        assert_eq!(common.get("reliability"), Some(&serde_json::json!(1234)));
        assert_eq!(
            common.get("reliability_spd_dec"),
            Some(&serde_json::json!(17))
        );
        assert_eq!(
            common.get("max_age"),
            Some(&serde_json::json!((12 + 5) * 366))
        );
        assert_eq!(
            common.get("date_of_last_service"),
            Some(&serde_json::json!(economy))
        );
        assert_eq!(
            common.get("date_of_last_service_newgrf"),
            Some(&serde_json::json!(calendar))
        );
        assert_eq!(
            common
                .get("vehicle_flags")
                .and_then(serde_json::Value::as_u64)
                .ok_or("flags")?
                & 4,
            4
        );
        let width = runtime.world.map().width();
        let mut max_height = 0;
        for index in [
            tile,
            tile.saturating_add(1),
            tile.saturating_add(width),
            tile.saturating_add(width).saturating_add(1),
        ] {
            max_height = max_height.max(
                runtime
                    .world
                    .map()
                    .tiles()
                    .get(usize::try_from(index)?)
                    .ok_or("corner")?
                    .height(),
            );
        }
        assert_eq!(
            common.get("z_pos"),
            Some(&serde_json::json!(u16::from(max_height) * 8))
        );
    }
    Ok(())
}
