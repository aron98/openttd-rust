use super::purchase_tests::fixture::{fixture, random, request};
use super::*;
use crate::{Command, CommandMode, CommandRequest, CommandReturn};
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};
mod admission;
mod publication;
mod serialization;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn sale(tile: u32, vehicle: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::SellVehicle {
            location: tile,
            vehicle,
            sell_chain: false,
            backup_order: false,
            client_id: 0,
        },
    }
}
fn bought(receipt: crate::CommandReceipt) -> Result<u32> {
    let Some(CommandReturn::Vehicle { vehicle, .. }) = receipt.returns.ok_or("returns")?.result
    else {
        return Err("vehicle result".into());
    };
    Ok(vehicle)
}
fn common(id: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: id,
        path: vec![
            PathElement::Field("roadveh".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field(name.into()),
        ],
        value,
    }
}
#[test]
fn sale_refunds_and_reuses_identity_without_resetting_surviving_caches() -> Result {
    let (mut runtime, tile) = fixture()?;
    let first = bought(runtime.execute_command(&request(tile))?)?;
    let second = bought(runtime.execute_command(&request(tile))?)?;
    let survivor = runtime
        .road
        .get(&VehicleId::new(second))
        .ok_or("survivor")?
        .clone();
    let before_groups = runtime.road_group_counts()?;
    let rng = random(runtime.world())?;
    let cash = crate::world_access::signed(runtime.world(), b"PLYR", 0, "money")?;
    let cost = runtime
        .execute_command(&sale(tile, first))?
        .exec
        .ok_or("sale execution")?;
    assert!(cost.success);
    assert!(cost.cost < 0);
    assert_eq!(
        crate::world_access::signed(runtime.world(), b"PLYR", 0, "money")?,
        cash.saturating_sub(cost.cost)
    );
    assert_eq!(random(runtime.world())?, rng);
    assert!(!runtime.road.contains_key(&VehicleId::new(first)));
    let after_groups = runtime.road_group_counts()?;
    assert_eq!(
        after_groups.get(&0).ok_or("company")?.all.vehicles,
        before_groups.get(&0).ok_or("company")?.all.vehicles - 1
    );
    assert_eq!(
        after_groups
            .get(&0)
            .ok_or("company")?
            .default_group
            .engines
            .get(&116),
        Some(&1)
    );
    assert_eq!(runtime.road.get(&VehicleId::new(second)), Some(&survivor));
    assert_eq!(bought(runtime.execute_command(&request(tile))?)?, first);
    assert_eq!(
        SavedVehicleView::new(runtime.world(), VehicleId::new(first))?.unit_number()?,
        1
    );
    Ok(())
}
#[test]
fn sale_estimate_preserves_world_allocator_and_caches() -> Result {
    let (mut runtime, tile) = fixture()?;
    let id = bought(runtime.execute_command(&request(tile))?)?;
    let before = runtime.world.saved_json()?;
    let allocation = runtime.allocation.clone();
    let caches = runtime.road.clone();
    let mut command = sale(tile, id);
    command.mode = CommandMode::Estimate;
    let receipt = runtime.execute_command(&command)?;
    assert!(receipt.result.ok_or("result")?.success);
    assert!(receipt.exec.is_none());
    assert_eq!(runtime.world.saved_json()?, before);
    assert_eq!(runtime.allocation, allocation);
    assert_eq!(runtime.road, caches);
    Ok(())
}
