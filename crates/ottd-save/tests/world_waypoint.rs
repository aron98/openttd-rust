//! Real native buoy/waypoint discrimination and pointer-subtype regressions.
#![cfg(test)]
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldError},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn fixture() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/storage-payment-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
#[test]
fn restores_real_native_buoy_as_waypoint() -> Result {
    let world = fixture()?;
    let state = world.saved_json()?;
    assert_eq!(
        state.pointer("/chunks/STNN/records/8/facilities"),
        Some(&serde_json::json!(144))
    );
    assert_eq!(
        state.pointer("/chunks/STNN/records/8/normal"),
        Some(&serde_json::json!([]))
    );
    assert!(
        !world
            .derived()
            .road_stop_chains
            .iter()
            .any(|chain| chain.station == 8)
    );
    Ok(())
}
#[test]
fn rejects_industry_station_pointer_to_real_waypoint() -> Result {
    let mut world = fixture()?;
    let result = world.edit_field(
        *b"INDY",
        0,
        &[PathElement::Field("neutral_station".into())],
        WireValue::Unsigned(9),
    );
    assert!(matches!(
        result,
        Err(WorldError::Invalid {
            reason: "station reference points to waypoint",
            ..
        })
    ));
    Ok(())
}
