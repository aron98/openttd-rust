use super::{Result, io};
use ottd_save::{Compression, world::WorldEdit};
use std::path::PathBuf;

#[test]
#[ignore = "CI preparation only; source and output must be explicit fresh paths"]
fn prepare_sparse_town_fixture() -> Result {
    let input = PathBuf::from(std::env::var("TREE_PREP_INPUT")?);
    let mut world = io::load(&input)?;
    let town = world
        .tables()
        .get(b"CITY")
        .ok_or("CITY")?
        .records()
        .get(&0)
        .ok_or("town0")?
        .clone();
    world.edit_batch(vec![WorldEdit::InsertRecord {
        chunk: *b"CITY",
        record: 2,
        value: town,
    }])?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(std::env::var("TREE_PREP_OUTPUT")?)?;
    std::io::Write::write_all(&mut file, &world.encode(Compression::None)?)?;
    Ok(())
}
