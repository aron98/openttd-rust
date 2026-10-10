//! Read-only road restoration preserves authoritative saved state.
use ottd_save::{Savegame, world::World};
use ottd_sim::runtime::{SimulationRuntime, VehicleId};

#[test]
fn empty_world_restoration_preserves_every_saved_field() -> Result<(), Box<dyn std::error::Error>> {
    let world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let before = world.saved_json()?;
    let runtime = SimulationRuntime::restore_vanilla(world)?;
    assert_eq!(runtime.world().saved_json()?, before);
    assert!(runtime.road_cache(VehicleId::new(0)).is_err());
    assert_eq!(runtime.content().engines().len(), 256);
    assert_eq!(runtime.into_world().saved_json()?, before);
    Ok(())
}

#[test]
fn unsupported_vehicle_and_content_families_reject() -> Result<(), Box<dyn std::error::Error>> {
    for bytes in [
        include_bytes!("../../../fixtures/world/populated-v362.sav").as_slice(),
        include_bytes!("../../../fixtures/world/modded-v362.sav").as_slice(),
    ] {
        let world = World::decode(&Savegame::decode(bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
        assert!(SimulationRuntime::restore_vanilla(world).is_err());
    }
    Ok(())
}

#[test]
fn dynamic_engine_view_uses_saved_values_not_catalog_defaults()
-> Result<(), Box<dyn std::error::Error>> {
    use ottd_save::{WireValue, world::PathElement};
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/replay/clear-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    world.edit_field(
        *b"ENGN",
        0,
        &[PathElement::Field("reliability".into())],
        WireValue::Unsigned(1234),
    )?;
    world.edit_field(
        *b"ENGN",
        0,
        &[PathElement::Field("reliability_spd_dec".into())],
        WireValue::Unsigned(27),
    )?;
    world.edit_field(
        *b"ENGN",
        0,
        &[PathElement::Field("age".into())],
        WireValue::Signed(456),
    )?;
    let before = world.saved_json()?;
    let runtime = SimulationRuntime::restore_vanilla(world)?;
    assert_eq!(runtime.engine(0)?.reliability()?, 1234);
    assert_eq!(runtime.engine(0)?.reliability_decay()?, 27);
    assert_eq!(runtime.engine(0)?.age()?, 456);
    assert!(runtime.engine(u16::MAX).is_err());
    assert_eq!(runtime.world().saved_json()?, before);
    Ok(())
}
