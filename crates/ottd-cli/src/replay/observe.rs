use super::envelope::Envelope;
use anyhow::{Result, ensure};
use ottd_save::{Compression, world::World};
use ottd_sim::{ReplayCursor, ReplayEvent, ReplayObservation, ReplayRuntime};
use serde::Serialize;
use std::{fs::File, io::Write, path::Path};
#[derive(Serialize)]
struct Checkpoint {
    label: String,
    runtime: ReplayRuntime,
}
#[derive(Serialize)]
struct Results {
    schema_version: u32,
    actions: Vec<ReplayObservation>,
    checkpoints: Vec<Checkpoint>,
}
pub(super) struct Stage {
    directory: tempfile::TempDir,
    limit: usize,
    total: usize,
    results: Results,
}
impl Stage {
    pub(super) fn new(parent: &Path, limit: usize) -> Result<Self> {
        Ok(Self {
            directory: tempfile::Builder::new()
                .prefix(".ottd-replay-")
                .tempdir_in(parent)?,
            limit,
            total: 0,
            results: Results {
                schema_version: 1,
                actions: Vec::new(),
                checkpoints: Vec::new(),
            },
        })
    }
    pub(super) fn path(&self) -> &Path {
        self.directory.path()
    }
    pub(super) fn observe(&mut self, event: &ReplayEvent<'_>, world: &World) -> Result<()> {
        match event {
            ReplayEvent::Action(action) => self.results.actions.push((*action).clone()),
            ReplayEvent::Checkpoint { label, runtime } => {
                self.write(&format!("{label}.sav"), &world.encode(Compression::Zlib)?)?;
                self.write(
                    &format!("{label}.world.json"),
                    &serde_json::to_vec(&world.saved_json()?)?,
                )?;
                self.write(
                    &format!("{label}.derived.json"),
                    &serde_json::to_vec(world.derived())?,
                )?;
                self.write(
                    &format!("{label}.runtime.json"),
                    &serde_json::to_vec(
                        &serde_json::json!({"before_save":runtime,"after_save":runtime}),
                    )?,
                )?;
                self.results.checkpoints.push(Checkpoint {
                    label: (*label).into(),
                    runtime: runtime.clone(),
                });
            }
        }
        Ok(())
    }
    pub(super) fn finish(&mut self, cursor: ReplayCursor) -> Result<()> {
        let save = std::fs::read(self.path().join("final.sav"))?;
        self.write(
            "checkpoint.json",
            &serde_json::to_vec(&Envelope::new(&save, cursor)?)?,
        )?;
        self.write("results.json", &serde_json::to_vec(&self.results)?)?;
        Ok(())
    }
    fn write(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        ensure!(
            bytes.len() <= self.limit,
            "replay artifact {name} exceeds byte limit"
        );
        self.total = self
            .total
            .checked_add(bytes.len())
            .ok_or_else(|| anyhow::anyhow!("replay output size overflow"))?;
        ensure!(
            self.total <= 512 * 1024 * 1024,
            "replay output exceeds 512 MiB"
        );
        let mut file = File::create_new(self.path().join(name))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }
}
