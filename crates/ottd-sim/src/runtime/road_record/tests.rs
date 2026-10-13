use super::{RoadBuildState, build};
use ottd_save::{
    Savegame,
    world::{World, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn world() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
const fn state() -> RoadBuildState {
    RoadBuildState {
        owner: 0,
        unit: 7,
        tile: 1,
        x: 24,
        y: 8,
        z: 16,
        direction: 1,
        engine: 116,
        image: 0,
        cargo: 0,
        capacity: 31,
        reliability: 45678,
        reliability_decay: 80,
        max_age: 4392,
        economy_date: 701_265,
        calendar_date: 701_300,
        build_year: 1920,
        service_interval: 150,
        service_percent: true,
        preview: true,
        value: 4811,
        random_bits: 0xabcd,
    }
}
#[test]
fn constructor_creates_complete_native_road_record_without_existing_vehicle() -> Result {
    let mut world = world()?;
    assert!(
        world
            .tables()
            .get(b"VEHS")
            .ok_or("VEHS")?
            .records()
            .is_empty()
    );
    let record = build(
        world.tables().get(b"VEHS").ok_or("VEHS")?.schema(),
        state(),
        0,
    )?;
    world.edit_batch(vec![WorldEdit::InsertRecord {
        chunk: *b"VEHS",
        record: 0,
        value: record,
    }])?;
    let json = world.saved_json()?;
    let common = json
        .pointer("/chunks/VEHS/records/0/roadveh/0/common/0")
        .ok_or("common")?;
    for (key, value) in [
        ("current_order.refit_cargo", 254),
        ("current_order.max_speed", 65535),
        ("cargo_age_counter", 1),
        ("dest_tile", u64::from(u32::MAX)),
        ("next", 0),
        ("orders", 0),
        ("next_shared", 0),
        ("group_id", 65534),
        ("last_station_visited", 65535),
        ("vehstatus", 11),
        ("subtype", 1),
        ("random_bits", 0xabcd),
        ("vehicle_flags", 0x204),
        ("unitnumber", 7),
        ("service_interval", 150),
    ] {
        assert_eq!(common.get(key), Some(&serde_json::json!(value)), "{key}");
    }
    assert_eq!(
        common.get("cargo.action_counts"),
        Some(&serde_json::json!([0, 0, 0, 0]))
    );
    assert_eq!(common.get("cargo.packets"), Some(&serde_json::json!([])));
    assert_eq!(
        common.get("date_of_last_service"),
        Some(&serde_json::json!(701_265))
    );
    assert_eq!(
        common.get("date_of_last_service_newgrf"),
        Some(&serde_json::json!(701_300))
    );
    assert_eq!(
        json.pointer("/chunks/VEHS/records/0/roadveh/0/state"),
        Some(&serde_json::json!(254))
    );
    let restored = World::decode(&world.to_savegame()?)?;
    assert_eq!(restored.saved_json()?, json);
    Ok(())
}
#[test]
fn constructor_rejects_incomplete_schema_instead_of_filling_unknown_defaults() -> Result {
    let world = world()?;
    let schema = world.tables().get(b"DATE").ok_or("DATE")?.schema();
    assert!(build(schema, state(), 0).is_err());
    Ok(())
}

#[test]
fn creation_cache_reads_candidate_and_preserves_constructor_transients() -> Result {
    use crate::{
        content::ContentCatalog,
        runtime::{SavedVehicleView, VehicleId, road_cache},
    };
    use ottd_save::{TileRawParts, WireValue, world::PathElement};
    for realistic in [false, true] {
        let mut world = world()?;
        let content = ContentCatalog::from_world(&world)?;
        let mut tile = TileRawParts::from(world.map().tiles().get(1).ok_or("tile")?);
        tile.tile_type = 0x20;
        tile.m4 = 0;
        tile.m5 = 0;
        let record = build(
            world.tables().get(b"VEHS").ok_or("VEHS")?.schema(),
            state(),
            0,
        )?;
        let before = world.saved_json()?;
        let mut transaction = world.transaction();
        transaction.apply(WorldEdit::InsertRecord {
            chunk: *b"VEHS",
            record: 0,
            value: record,
        })?;
        transaction.apply(WorldEdit::Tile {
            index: 1,
            value: tile.into(),
        })?;
        transaction.apply(WorldEdit::Field {
            chunk: *b"PATS",
            record: 0,
            path: vec![PathElement::Field(
                "vehicle.roadveh_acceleration_model".into(),
            )],
            value: WireValue::Unsigned(u64::from(realistic)),
        })?;
        let prepared = transaction.prepare()?;
        let candidate = SavedVehicleView::candidate(prepared.view(), VehicleId::new(0))?;
        let cache = road_cache::create(candidate, &content)?;
        assert_eq!(cache.last_speed, 0);
        assert_eq!(cache.trip_occupancy, 0);
        assert_eq!(cache.power > 0, realistic);
        assert_eq!(cache.weight > 0, realistic);
        assert!(cache.max_speed > 0);
        assert_eq!(cache.vehicle_length, 8);
        assert!(road_cache::restore(candidate, &content)?.power > 0);
        drop(prepared);
        assert_eq!(world.saved_json()?, before);
    }
    Ok(())
}
