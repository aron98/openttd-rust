//! Pinned vanilla gameplay specifications and derived prices.
mod aircraft_data;
mod cargo;
mod cargo_data;
mod engine_data;
mod engines;
mod price;
mod price_data;
mod prices;
mod rail_data;
mod road_data;
mod ship_data;
mod types;
mod vehicle;
mod world;
pub use cargo::CargoSpec;
pub use price::Price;
pub use prices::{Difficulty, PriceSettings, Prices};
use serde::Serialize;
pub use types::{CargoLabelSource, Climate, EngineInfo, EngineSpec, VehicleSpec};
pub use vehicle::{AircraftSpec, RailSpec, RoadSpec, ShipSpec};

/// Content initialization failed without substituting unsupported definitions.
#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    /// A setting or saved mapping needs content behavior outside this catalog.
    #[error("unsupported content: {0}")]
    Unsupported(&'static str),
    /// Inflation exceeds the pinned native limit.
    #[error("inflation exceeds native MAX_INFLATION")]
    Inflation,
    /// Arithmetic or static table structure is invalid.
    #[error("content arithmetic or table invariant failed")]
    Arithmetic,
    /// The saved world has an unexpected field shape.
    #[error("saved content input: {0}")]
    Saved(String),
}

/// Immutable initialized specifications, separate from saved dynamic ENGN state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContentCatalog {
    climate: Climate,
    engines: Vec<EngineSpec>,
    cargo: Vec<CargoSpec>,
    prices: Prices,
}
impl ContentCatalog {
    /// Initialize all original engine/cargo definitions with electric rail enabled.
    ///
    /// # Errors
    /// Rejects inflation beyond the native limit or arithmetic overflow.
    pub fn vanilla(climate: Climate, settings: PriceSettings) -> Result<Self, ContentError> {
        let prices = Prices::new(settings)?;
        let mut cargo = vec![CargoSpec::EMPTY; 64];
        let indices = cargo_data::CLIMATE_CARGO
            .get(usize::from(climate as u8))
            .ok_or(ContentError::Arithmetic)?;
        for (slot, &index) in cargo.iter_mut().zip(indices) {
            *slot = *cargo_data::CARGO
                .get(index)
                .ok_or(ContentError::Arithmetic)?;
        }
        let inflation =
            i64::try_from(settings.inflation_payment).map_err(|_| ContentError::Inflation)?;
        for spec in cargo.iter_mut().filter(|c| c.bitnum != 255) {
            spec.current_payment = i64::from(spec.initial_payment)
                .checked_mul(inflation)
                .ok_or(ContentError::Arithmetic)?
                >> 16;
        }
        let engines = engines::initialize(climate, &cargo)?;
        Ok(Self {
            climate,
            engines,
            cargo,
            prices,
        })
    }
    /// Every original engine in native global-ID order.
    pub fn engines(&self) -> &[EngineSpec] {
        &self.engines
    }
    /// All 64 native cargo slots, including invalid slots.
    pub fn cargo(&self) -> &[CargoSpec] {
        &self.cargo
    }
    /// Current vanilla base price in native money units.
    pub fn price(&self, price: Price) -> i64 {
        self.prices.get(price)
    }
    /// Computed prices and loan limit.
    pub const fn prices(&self) -> &Prices {
        &self.prices
    }
    /// Apply the native electric-rail setting without altering intended railtypes.
    #[must_use]
    pub fn with_electric_rail(mut self, enabled: bool) -> Self {
        for engine in &mut self.engines {
            match &mut engine.vehicle {
                VehicleSpec::Rail(rail) if rail.intended_railtypes & 2 != 0 => {
                    rail.railtypes = if enabled {
                        rail.intended_railtypes
                    } else {
                        (rail.intended_railtypes & !2) | 1
                    };
                }
                VehicleSpec::Rail(_)
                | VehicleSpec::Road(_)
                | VehicleSpec::Ship(_)
                | VehicleSpec::Aircraft(_) => {}
            }
        }
        self
    }
}
