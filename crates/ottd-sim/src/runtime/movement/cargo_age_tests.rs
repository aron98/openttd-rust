//! Synthetic saturation/countdown controls start from genuine loaded native cargo.
use super::{RoadPhases, State, number, set, spatial, stations};
use crate::runtime::SimulationRuntime;
use ottd_save::{Savegame, world::World};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires genuine loaded CARGO_AGE_INPUT and CARGO_AGE_OUTPUT"]
fn packet_age_saturation_countdown_and_candidate_rollback() -> Result {
    let input = std::env::var("CARGO_AGE_INPUT")?;
    let output = std::path::PathBuf::from(std::env::var("CARGO_AGE_OUTPUT")?);
    let bytes = std::fs::read(input)?;
    let mut results = Vec::new();
    for (age, counter, period, expected_age, expected_counter) in [
        (0, 1, 185, 1, 185),
        (65534, 1, 185, 65535, 185),
        (65535, 1, 185, 65535, 185),
        (0, 0, 185, 0, 65535),
        (0, 250, 185, 0, 184),
        (0, 1, 0, 0, 1),
    ] {
        let world = World::decode(&Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        let original = runtime.saved_json()?;
        let original_cargo = runtime.transport.cargo.clone();
        let (&id, cache) = runtime.road.first_key_value().ok_or("road cache")?;
        let mut runner = RoadPhases {
            id,
            cache: cache.clone(),
            content: &runtime.content,
            spatial: spatial::Spatial::restore(&runtime.world, id)?,
            stations: stations::IdleStations::restore(&runtime.world, &runtime.stations)?,
            station_context: &runtime.stations,
            transport: runtime.transport.clone(),
        };
        runner.cache.cargo_age_period = period;
        {
            let industries = crate::runtime::IndustryCaches::restore(&runtime.world)?;
            let mut state = State::load(&mut runtime.world, None, industries);
            set(&mut state, id, false, "cargo_age_counter", counter)?;
            let list = runner
                .transport
                .cargo
                .vehicles
                .get_mut(&id.raw())
                .ok_or("cargo")?;
            assert!(list.count > 0);
            for packet in &list.packets {
                state.set_number(b"CAPA", *packet, "periods_in_transit", age)?;
            }
            list.periods = u64::from(list.count) * u64::try_from(age)?;
            runner.age_cargo(&mut state)?;
            let list = runner
                .transport
                .cargo
                .vehicles
                .get(&id.raw())
                .ok_or("cargo")?;
            for packet in &list.packets {
                assert_eq!(
                    state.number(b"CAPA", *packet, "periods_in_transit")?,
                    expected_age
                );
            }
            assert_eq!(
                list.periods,
                u64::from(list.count) * u64::try_from(expected_age)?
            );
            assert_eq!(
                number(&state, id, false, "cargo_age_counter")?,
                expected_counter
            );
            runner.transport.cargo.validate(state.view())?;
            results.push(
                serde_json::json!({"synthetic":true,"age":age,"counter":counter,
                "period":period,"result_age":expected_age,"result_counter":expected_counter,
                "full_candidate_cargo_validated":true}),
            );
            // The positive age mutation remains tentative when the candidate is dropped.
        }
        assert_eq!(runtime.saved_json()?, original);
        assert_eq!(runtime.transport.cargo, original_cargo);
    }
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&results)?,
    )?;
    Ok(())
}
