use super::*;
pub(super) fn sale(tile: u32, vehicle: u32, backup_order: bool, client_id: u32) -> CommandRequest {
    CommandRequest {
        company: 0,
        mode: CommandMode::Post,
        command: Command::SellVehicle {
            location: tile,
            vehicle,
            sell_chain: false,
            backup_order,
            client_id,
        },
    }
}
pub(super) fn sole() -> Result<(SimulationRuntime, u32, u32)> {
    let (mut runtime, tile, _, first, second) = super::super::tests::shared_fixture()?;
    assert!(
        runtime
            .execute_command(&sale(tile, second, false, 77))?
            .posted
    );
    Ok((runtime, tile, first))
}
pub(super) fn capacity_sale(user: u32) -> Result<(SimulationRuntime, Vec<ottd_save::TableRecord>)> {
    let (mut runtime, tile, _, first, second) = super::super::tests::shared_fixture()?;
    for user in 1..=255 {
        runtime.backup_orders(VehicleId::new(first), user)?;
    }
    assert!(
        runtime
            .execute_command(&sale(tile, first, false, 77))?
            .posted
    );
    assert_eq!(runtime.order_backup_pool().items, 255);
    let expected = view::row(runtime.world(), *b"ORDL", 60000)?
        .children("orders")?
        .1
        .to_vec();
    assert!(
        runtime
            .execute_command(&sale(tile, second, true, user))?
            .posted
    );
    Ok((runtime, expected))
}
pub(super) fn field(vehicle: u32, name: &str, value: ottd_save::WireValue) -> WorldEdit {
    use ottd_save::world::PathElement::{Field, Index};
    WorldEdit::Field {
        chunk: *b"VEHS",
        record: vehicle,
        path: vec![
            Field("roadveh".into()),
            Index(0),
            Field("common".into()),
            Index(0),
            Field(name.into()),
        ],
        value,
    }
}
