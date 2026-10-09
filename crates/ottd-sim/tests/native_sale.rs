//! Typed setup for original-command sale fixtures; no vehicle deletion here.
use ottd_save::{
    Compression, Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::{
    content::{ContentCatalog, VehicleSpec},
    runtime::{SimulationRuntime, VehicleId},
};
use std::path::{Path, PathBuf};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn field(chunk: [u8; 4], name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
fn vehicle(id: u32, name: &str, value: WireValue) -> WorldEdit {
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
fn read(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn write(world: &World, output: &Path, name: &str) -> Result {
    std::fs::write(
        output.join(format!("{name}.sav")),
        world.to_savegame()?.encode(Compression::None)?,
    )?;
    std::fs::write(
        output.join(format!("{name}.world.json")),
        serde_json::to_vec(&world.saved_json()?)?,
    )?;
    std::fs::write(
        output.join(format!("{name}.derived.json")),
        serde_json::to_vec(world.derived())?,
    )?;
    Ok(())
}
#[path = "native_sale/cleanup.rs"]
mod cleanup;
#[path = "native_sale/inputs.rs"]
mod inputs;
