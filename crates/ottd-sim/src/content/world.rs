use super::{Climate, ContentCatalog, ContentError, Difficulty, PriceSettings, VehicleSpec};
use crate::world_access::unsigned;
use ottd_save::world::World;

impl ContentCatalog {
    /// Restore vanilla specifications from saved settings and default engine mappings.
    ///
    /// # Errors
    /// Rejects `NewGRFs`, non-default engine IDs, invalid settings and inflation.
    pub fn from_world(world: &World) -> Result<Self, ContentError> {
        if world
            .tables()
            .get(b"NGRF")
            .is_none_or(|table| !table.records().is_empty())
        {
            return Err(ContentError::Unsupported("NewGRF configuration"));
        }
        let setting = |name| number(world, *b"PATS", 0, name);
        let climate = match setting("game_creation.landscape")? {
            0 => Climate::Temperate,
            1 => Climate::Arctic,
            2 => Climate::Tropic,
            3 => Climate::Toyland,
            _ => return Err(ContentError::Unsupported("landscape")),
        };
        let difficulty = |name| match setting(name)? {
            0 => Ok(Difficulty::Low),
            1 => Ok(Difficulty::Medium),
            2 => Ok(Difficulty::High),
            _ => Err(ContentError::Unsupported("difficulty")),
        };
        let settings = PriceSettings {
            construction: difficulty("difficulty.construction_cost")?,
            running: difficulty("difficulty.vehicle_costs")?,
            inflation_prices: number(world, *b"ECMY", 0, "inflation_prices")?,
            inflation_payment: number(world, *b"ECMY", 0, "inflation_payment")?,
            max_loan: u32::try_from(setting("difficulty.max_loan")?)
                .map_err(|_| ContentError::Arithmetic)?,
        };
        let catalog = Self::vanilla(climate, settings)?
            .with_electric_rail(setting("vehicle.disable_elrails")? == 0);
        let mapping = world
            .tables()
            .get(b"EIDS")
            .ok_or(ContentError::Unsupported("missing engine mapping"))?;
        if mapping.records().len() != catalog.engines.len() {
            return Err(ContentError::Unsupported("engine mapping cardinality"));
        }
        for engine in &catalog.engines {
            let id = u32::from(engine.id);
            let kind = match engine.vehicle {
                VehicleSpec::Rail(_) => 0,
                VehicleSpec::Road(_) => 1,
                VehicleSpec::Ship(_) => 2,
                VehicleSpec::Aircraft(_) => 3,
            };
            if number(world, *b"EIDS", id, "grfid")? != u64::from(u32::MAX)
                || number(world, *b"EIDS", id, "internal_id")? != u64::from(engine.local_id)
                || number(world, *b"EIDS", id, "substitute_id")? != u64::from(engine.local_id)
                || number(world, *b"EIDS", id, "type")? != kind
            {
                return Err(ContentError::Unsupported("non-default engine mapping"));
            }
        }
        Ok(catalog)
    }
}
fn number(world: &World, chunk: [u8; 4], id: u32, name: &str) -> Result<u64, ContentError> {
    unsigned(world, &chunk, id, name).map_err(|error| ContentError::Saved(error.to_string()))
}
