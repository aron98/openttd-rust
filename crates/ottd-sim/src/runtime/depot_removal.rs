//! One publication for depot destruction and its canonical order lifecycle.
use super::{DepotContext, RuntimeError, order_state::OrderReader};
use crate::{CommandError, world_access::unsigned};
use ottd_save::world::{World, WorldEdit};

impl DepotContext<'_> {
    pub(crate) fn publish_removal(
        self,
        world: &mut World,
        tile: u32,
        mut edits: Vec<WorldEdit>,
    ) -> Result<(), CommandError> {
        let source = world
            .map()
            .tiles()
            .get(usize::try_from(tile).map_err(|_| RuntimeError::Invalid("depot tile"))?)
            .ok_or(RuntimeError::Invalid("depot tile"))?;
        let depot = source.m2();
        if source.tile_type() >> 4 != 2
            || source.m5() >> 6 != 2
            || unsigned(world, b"DEPT", u32::from(depot), "xy")? != u64::from(tile)
        {
            return Err(RuntimeError::Invalid("depot removal identity").into());
        }
        let road = source.m4() & 63;
        let tram = u8::try_from((source.m8() >> 6) & 63)
            .map_err(|_| RuntimeError::Invalid("depot tram type"))?;
        let kind = match (road, tram) {
            (0, 63) => 0,
            (63, 1) => 1,
            _ => return Err(CommandError::Unsupported("vanilla depot road type")),
        };
        let counter = self
            .road
            .get_mut(&(source.m1() & 31))
            .and_then(|counts| counts.get_mut(kind))
            .ok_or(RuntimeError::Invalid("depot infrastructure"))?;
        let count = counter
            .checked_sub(2)
            .ok_or(RuntimeError::Invalid("depot infrastructure underflow"))?;
        let mut pool = self.pool.clone();
        pool.free(u32::from(depot)).map_err(RuntimeError::from)?;
        let order_plan = self.orders.plan_depot_invalidation(
            OrderReader::Committed(world),
            &world.derived().order_lists,
            depot,
            tile,
            world.map().width(),
            world.map().height(),
        )?;
        edits.push(WorldEdit::RemoveRecord {
            chunk: *b"DEPT",
            record: u32::from(depot),
        });
        let mut transaction = world.transaction();
        let pending = order_plan.stage(&mut transaction)?;
        for edit in edits {
            transaction.apply(edit)?;
        }
        let prepared = transaction.prepare()?;
        let ids: Vec<_> = prepared
            .view()
            .table(*b"DEPT")
            .ok_or(RuntimeError::Invalid("DEPT"))?
            .records()
            .map(|(id, _)| id)
            .collect();
        if ids != pool.snapshot().occupied {
            return Err(RuntimeError::Invalid("depot candidate pool membership").into());
        }
        let (prepared, orders) = pending.validate(prepared)?;
        prepared.commit();
        orders.publish(self.orders);
        *self.pool = pool;
        *counter = count;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Command, CommandMode, CommandRequest, CommandReturn,
        runtime::{
            RuntimeSaveContext, SimulationRuntime, VehicleId,
            purchase_tests::fixture::{fixture, request},
        },
    };
    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
    #[test]
    fn occupied_depot_keeps_live_same_tile_backups_and_construction_expense() -> Result {
        let (mut runtime, tile) = fixture()?;
        let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
            .execute_command(&request(tile))?
            .returns
            .ok_or("return")?
            .result
        else {
            return Err("vehicle".into());
        };
        runtime.backup_orders(VehicleId::new(vehicle), 100)?;
        let saved = runtime.world.saved_json()?;
        let orders = runtime.order_state_json()?;
        let depots = runtime.depot.clone();
        let result = runtime.execute_command(&CommandRequest {
            company: 0,
            mode: CommandMode::Post,
            command: Command::LandscapeClear { tile },
        })?;
        let cost = result.result.ok_or("result")?;
        assert_eq!(
            cost.error.as_deref(),
            Some("STR_ERROR_ROAD_VEHICLE_IN_THE_WAY")
        );
        assert_eq!(cost.expenses, 0);
        assert!(result.exec.is_none());
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(runtime.order_state_json()?, orders);
        assert_eq!(runtime.depot, depots);
        Ok(())
    }
    #[test]
    fn transaction_rejects_loaded_native_backup_index_before_any_publication() -> Result {
        let (mut runtime, tile) = fixture()?;
        let Some(CommandReturn::Vehicle { vehicle, .. }) = runtime
            .execute_command(&request(tile))?
            .returns
            .ok_or("return")?
            .result
        else {
            return Err("vehicle".into());
        };
        for user in [100, 101] {
            runtime.backup_orders(VehicleId::new(vehicle), user)?;
        }
        let (mut runtime, _) = SimulationRuntime::from_loaded_vanilla(
            runtime.into_world(),
            RuntimeSaveContext::NetworkClient,
        )?;
        let saved = runtime.world.saved_json()?;
        let derived = serde_json::to_value(runtime.world.derived())?;
        let orders = runtime.order_state_json()?;
        let depots = runtime.depot.clone();
        // Exercise the transaction seam directly: public client commands remain unsupported.
        let result = DepotContext {
            orders: &mut runtime.orders,
            content: &runtime.content,
            pool: &mut runtime.depot.pool,
            road: &mut runtime.depot.road,
        }
        .publish_removal(&mut runtime.world, tile, Vec::new());
        assert!(matches!(
            result,
            Err(CommandError::Runtime(RuntimeError::NativeBackupIndex {
                slot: 1,
                index: 0
            }))
        ));
        assert_eq!(runtime.world.saved_json()?, saved);
        assert_eq!(serde_json::to_value(runtime.world.derived())?, derived);
        assert_eq!(runtime.order_state_json()?, orders);
        assert_eq!(runtime.depot, depots);
        Ok(())
    }
}
