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
