//! Deterministic, explicitly scoped landscape simulation for OpenTTD 15.3.
mod landscape;
mod map;

pub use landscape::{Landscape, LandscapeError};
pub use map::{Map, Tile};
mod wire;
pub use wire::{Clock, Context, Fixture, State, TickEvents};
mod clock;
mod simulation;
pub use simulation::{MAX_TICKS, SimulationError, simulate};
mod vehicle_callbacks;
mod vehicle_types;
pub use vehicle_callbacks::{VehicleCallbackError, run_vehicle_callback};
pub use vehicle_types::{
    CallbackGroup, CallbackVehicle, VehicleCallbacks, VehicleKind, VehicleOperation,
};
mod callbacks;
pub use callbacks::{Callback, CallbackError, CallbackFixture, simulate_callback};
mod periodic_callbacks;
mod periodic_types;
pub use periodic_callbacks::{
    PeriodicCallbackError, run_company_year, run_house_year, run_station_month,
};
pub use periodic_types::{
    CompanyCallbacks, CompanyExpenses, HouseCallbacks, StationCallbacks, StationCargo,
    StationStatus,
};
mod industry_callbacks;
mod industry_history;
mod industry_types;
pub use industry_callbacks::{IndustryCallbackError, run_industry_month};
pub use industry_types::{
    AcceptedHistory, CallbackIndustry, IndustryAccepted, IndustryCallbacks, IndustryMonth,
    IndustryProduced, ProducedHistory,
};
