//! Packet admission belongs to a cargo consumer, not merely the job scheduler.
use super::validate;
use crate::{WorldTickError, WorldTickPhases, WorldTickState, runtime::SimulationRuntime};
use ottd_core::{ClockEvents, ClockState};
use ottd_save::{Savegame, world::World};

struct TransportOnly;
impl WorldTickPhases for TransportOnly {
    fn transport_activity(&self) -> bool {
        true
    }
    fn station_tiles(&self) -> bool {
        true
    }
    fn admit(&self, _: &World) -> Result<(), WorldTickError> {
        Ok(())
    }
    fn economy_boundary(&self, _: &ClockEvents) -> Result<(), WorldTickError> {
        Ok(())
    }
    fn calendar(
        &mut self,
        _: &mut WorldTickState<'_>,
        _: &ClockState,
        _: bool,
    ) -> Result<(), WorldTickError> {
        Ok(())
    }
    fn ticks(&mut self, _: &mut WorldTickState<'_>, _: &ClockState) -> Result<(), WorldTickError> {
        Ok(())
    }
}
#[test]
#[ignore = "requires genuine loaded CAPABILITY_INPUT and CAPABILITY_OUTPUT"]
fn transport_only_rejects_packets_and_real_cargo_consumer_advances()
-> Result<(), Box<dyn std::error::Error>> {
    let world = World::decode(&Savegame::decode(
        &std::fs::read(std::env::var("CAPABILITY_INPUT")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    assert!(
        !world
            .tables()
            .get(b"CAPA")
            .ok_or("packet table")?
            .records()
            .is_empty()
    );
    let error =
        validate(&world, false, &TransportOnly).expect_err("scheduler is not a packet consumer");
    assert!(format!("{error:?}").contains("nonempty CAPA"));
    assert!(!validate(&world, true, &TransportOnly)?);
    let mut runtime = SimulationRuntime::restore_vanilla(world)?;
    let before = format!("{runtime:?}");
    assert!(runtime.advance_world(0)?.ticks.is_empty());
    assert_eq!(format!("{runtime:?}"), before);
    assert_eq!(runtime.advance_world(1)?.ticks.len(), 1);
    let output = std::path::PathBuf::from(std::env::var("CAPABILITY_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    std::fs::write(
        output.join("PASS.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "transport_only_nonempty_CAPA_refusal":format!("{error:?}"),"paused_domain_preserved":true,
            "actual_runtime_zero_unchanged":true,"actual_cargo_consumer_advanced":1,
            "native_parity_credit":"separate original/Rust both-simulated-origin corpus"
        }))?,
    )?;
    Ok(())
}
