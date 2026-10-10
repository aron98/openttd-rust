use super::{Path, PathBuf, Result, Value, equal};
use ottd_save::{Savegame, world::World};
use serde::de::DeserializeOwned;
use std::fs::{File, OpenOptions};

pub(super) fn json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_reader(File::open(path)?)?)
}
pub(super) fn load(path: &Path) -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        &std::fs::read(path)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
pub(super) fn record(name: &str, value: &Value) -> Result {
    let root = PathBuf::from(std::env::var("TREE_CORPUS_OUTPUT")?);
    std::fs::create_dir_all(&root)?;
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(format!("{name}.json")))?;
    serde_json::to_writer_pretty(output, value)?;
    Ok(())
}
pub(super) fn checkpoint(world: &World, run: &Path, label: &str, lane: &str) -> Result {
    equal(
        &world.saved_json()?,
        &json(&run.join(format!("{label}.world.json")))?,
        lane,
        "saved world",
    )?;
    equal(
        &serde_json::to_value(world.derived())?,
        &json(&run.join(format!("{label}.derived.json")))?,
        lane,
        "derived state",
    )
}
