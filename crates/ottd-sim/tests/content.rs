//! Native content initialization, pricing and unsupported-input regressions.
use ottd_sim::content::{Climate, ContentCatalog, Difficulty, Price, PriceSettings, VehicleSpec};

#[test]
fn native_catalog_boundaries_and_refitting() -> Result<(), Box<dyn std::error::Error>> {
    for climate in [
        Climate::Temperate,
        Climate::Arctic,
        Climate::Tropic,
        Climate::Toyland,
    ] {
        let catalog = ContentCatalog::vanilla(climate, PriceSettings::default())?;
        assert_eq!(catalog.engines().len(), 256);
        assert_eq!(catalog.cargo().len(), 64);
        assert_eq!(
            catalog
                .cargo()
                .iter()
                .filter(|cargo| cargo.bitnum != 255)
                .count(),
            if matches!(climate, Climate::Temperate | Climate::Arctic) {
                11
            } else {
                12
            }
        );
        assert_eq!(catalog.price(Price::BuildRail), 100);
        let Some(first) = catalog.engines().first() else {
            return Err("empty engines".into());
        };
        assert_eq!(first.info.cargo_age_period, 185);
        assert_eq!(first.info.refit_mask, 0);
        assert!(matches!(first.vehicle, VehicleSpec::Rail(_)));
        let Some(aircraft) = catalog.engines().last() else {
            return Err("empty engines".into());
        };
        assert_ne!(aircraft.info.refit_mask, 0);
        assert_eq!(aircraft.info.cargo_type, 0);
    }
    Ok(())
}

#[test]
fn price_difficulty_signed_rounding_and_limits() -> Result<(), Box<dyn std::error::Error>> {
    let settings = PriceSettings {
        construction: Difficulty::Low,
        running: Difficulty::High,
        inflation_prices: 65_537,
        ..PriceSettings::default()
    };
    let catalog = ContentCatalog::vanilla(Climate::Temperate, settings)?;
    assert_eq!(catalog.price(Price::BuildRail), 75);
    assert_eq!(catalog.price(Price::ClearRail), -53);
    assert_eq!(catalog.price(Price::RunningRoadveh), 1800);
    assert_eq!(catalog.price(Price::StationValue), 100);
    for inflation in [0, 1, 65_536, 2_147_483_647] {
        let catalog = ContentCatalog::vanilla(
            Climate::Tropic,
            PriceSettings {
                inflation_prices: inflation,
                inflation_payment: inflation,
                ..settings
            },
        )?;
        assert_ne!(catalog.price(Price::ClearRail), 0);
        assert_ne!(catalog.price(Price::BuildRail), 0);
    }
    assert!(
        ContentCatalog::vanilla(
            Climate::Temperate,
            PriceSettings {
                inflation_prices: 2_147_483_648,
                ..settings
            }
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn climate_specific_duplicate_labels_keep_native_properties()
-> Result<(), Box<dyn std::error::Error>> {
    let temperate = ContentCatalog::vanilla(Climate::Temperate, PriceSettings::default())?;
    let tropic = ContentCatalog::vanilla(Climate::Tropic, PriceSettings::default())?;
    assert_eq!(
        temperate.cargo().get(3).map(|c| c.initial_payment),
        Some(4437)
    );
    assert_eq!(tropic.cargo().get(3).map(|c| c.initial_payment), Some(4892));
    assert_eq!(
        temperate.cargo().get(7).map(|c| c.initial_payment),
        Some(5005)
    );
    assert_eq!(tropic.cargo().get(7).map(|c| c.initial_payment), Some(7964));
    Ok(())
}

#[test]
fn saved_world_rejects_mods_and_non_default_engine_identity()
-> Result<(), Box<dyn std::error::Error>> {
    use ottd_save::{
        Savegame, WireValue,
        world::{PathElement, World},
    };
    let mut world = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/generated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let catalog = ContentCatalog::from_world(&world)?;
    assert_eq!(catalog.engines().len(), 256);
    world.edit_field(
        *b"EIDS",
        0,
        &[PathElement::Field("grfid".into())],
        WireValue::Unsigned(123),
    )?;
    assert!(ContentCatalog::from_world(&world).is_err());
    let modded = World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/modded-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    assert!(ContentCatalog::from_world(&modded).is_err());
    Ok(())
}

#[test]
fn disabling_electric_rail_preserves_intended_types() -> Result<(), Box<dyn std::error::Error>> {
    let original = ContentCatalog::vanilla(Climate::Temperate, PriceSettings::default())?;
    let modified = original.clone().with_electric_rail(false);
    let mut changed = 0_u16;
    for (before, after) in original.engines().iter().zip(modified.engines()) {
        if let (VehicleSpec::Rail(before), VehicleSpec::Rail(after)) =
            (before.vehicle, after.vehicle)
        {
            assert_eq!(before.intended_railtypes, after.intended_railtypes);
            if before.intended_railtypes & 2 != 0 {
                assert_eq!(after.railtypes & 3, 1);
                changed = changed.checked_add(1).ok_or("count")?;
            }
        }
    }
    assert!(changed > 0);
    assert_eq!(modified.with_electric_rail(true), original);
    Ok(())
}
