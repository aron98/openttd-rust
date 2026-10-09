mod admission;
mod constructor;
mod fixture;
mod publication;

use super::*;
use crate::{Command, CommandMode, CommandRequest};
use ottd_save::{
    Savegame, TileRawParts, WireValue,
    world::{PathElement, WorldEdit},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
use fixture::{fixture, random, request};
