use anyhow::{Context, Result, ensure};
use ottd_save::{Savegame, world::World};
use ottd_sim::ReplayCursor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::{fs::File, io::Read, path::Path};
const REVISION: &str = "14ec60f248547d4d062a1160f0fc26d742319888";
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    schema_version: u32,
    native_revision: String,
    savegame_version: u16,
    save: SaveIdentity,
    cursor: ReplayCursor,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveIdentity {
    file: String,
    sha256: String,
}
impl Envelope {
    pub(super) fn new(save: &[u8], cursor: ReplayCursor) -> Result<Self> {
        Ok(Self {
            schema_version: 1,
            native_revision: REVISION.into(),
            savegame_version: 362,
            save: SaveIdentity {
                file: "final.sav".into(),
                sha256: hash(save)?,
            },
            cursor,
        })
    }
}
fn hash(bytes: &[u8]) -> Result<String> {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}")?;
    }
    Ok(output)
}
pub(super) fn load(path: &Path, limit: usize) -> Result<(World, ReplayCursor)> {
    let envelope: Envelope = serde_json::from_value(crate::compare::load_json(
        path,
        limit.min(16 * 1024 * 1024),
    )?)
    .context("invalid replay checkpoint")?;
    ensure!(
        envelope.schema_version == 1
            && envelope.native_revision == REVISION
            && envelope.savegame_version == 362,
        "checkpoint compatibility identity mismatch"
    );
    ensure!(
        envelope.save.file == "final.sav",
        "checkpoint save must be the sibling final.sav"
    );
    ensure!(
        envelope.save.sha256.len() == 64
            && envelope
                .save
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "invalid save SHA-256"
    );
    envelope.cursor.validate()?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let save_path = parent.join(&envelope.save.file);
    let metadata = std::fs::symlink_metadata(&save_path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "checkpoint save must be a regular sibling file"
    );
    let mut bytes = Vec::new();
    File::open(&save_path)?
        .take(u64::try_from(limit)?.saturating_add(1))
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "checkpoint save exceeds byte limit");
    ensure!(
        hash(&bytes)? == envelope.save.sha256,
        "checkpoint save SHA-256 mismatch"
    );
    let world = World::decode(&Savegame::decode(&bytes, limit)?)?;
    Ok((world, envelope.cursor))
}
