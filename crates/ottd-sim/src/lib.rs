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
