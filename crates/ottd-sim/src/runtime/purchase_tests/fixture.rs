use super::*;

pub(in crate::runtime) fn fixture() -> Result<(SimulationRuntime, u32)> {
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let ids = world
        .tables()
        .get(b"VEHS")
        .ok_or("VEHS")?
        .records()
        .keys()
        .copied();
    let edits = ids
        .filter(|id| SavedVehicleView::new(&world, VehicleId::new(*id)).is_err())
        .map(|id| WorldEdit::RemoveRecord {
            chunk: *b"VEHS",
            record: id,
        })
        .collect();
    world.edit_batch(edits)?;
    let (index, tile) = world
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
        .ok_or("depot")?;
    let index = u32::try_from(index)?;
    let mut raw = TileRawParts::from(tile);
    raw.m1 = 0;
    world.edit_batch(vec![
        WorldEdit::Tile {
            index,
            value: raw.into(),
        },
        WorldEdit::Field {
            chunk: *b"ENGN",
            record: 116,
            path: vec![PathElement::Field("company_avail".into())],
            value: WireValue::Unsigned(1),
        },
        WorldEdit::Field {
            chunk: *b"DATE",
            record: 0,
            path: vec![PathElement::Field("pause_mode".into())],
            value: WireValue::Unsigned(0),
        },
    ])?;
    Ok((SimulationRuntime::restore_vanilla(world)?, index))
}

pub(in crate::runtime) fn request(tile: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::BuildVehicle {
            tile,
            engine: 116,
            cargo: 255,
            use_free_vehicles: false,
            client_id: 0,
        },
    }
}

pub(in crate::runtime) fn random(world: &World) -> Result<[u32; 2]> {
    Ok([
        u32::try_from(crate::world_access::unsigned(
            world,
            b"DATE",
            0,
            "random_state[0]",
        )?)?,
        u32::try_from(crate::world_access::unsigned(
            world,
            b"DATE",
            0,
            "random_state[1]",
        )?)?,
    ])
}
