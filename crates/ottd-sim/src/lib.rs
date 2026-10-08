//! Deterministic, explicitly scoped landscape simulation for OpenTTD 15.3.
mod landscape;
mod map;

pub use landscape::{Landscape, LandscapeError};
pub use map::{Map, Tile};
