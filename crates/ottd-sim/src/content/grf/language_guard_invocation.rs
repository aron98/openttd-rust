use super::Result;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(super) enum Guard {
    Replay,
    Subset,
    Reused,
    NonSave,
    Stale,
    MissingOutput,
    DuplicateOutput,
    Selection,
    Body(BodyKind),
    Unarmed,
}
pub(super) enum BodyKind {
    Truncated,
    Table,
    LastExtended,
    ReadCap,
}
impl BodyKind {
    fn change(&self, bytes: &mut Vec<u8>) -> Result {
        match self {
            Self::Truncated => bytes.truncate(574),
            Self::Table => bytes
                .get_mut(88..90)
                .ok_or("table field")?
                .copy_from_slice(&2049_u16.to_le_bytes()),
            Self::LastExtended => {
                bytes.truncate(super::super::bodies::last_record(bytes)?);
                bytes.extend([0xc0, 0]);
            }
            Self::ReadCap => bytes.resize((1 << 20) + 1, 0),
        }
        Ok(())
    }
}

fn stale(root: &Path, directory: &Path) -> Result<PathBuf> {
    let target = directory.join("stale-source");
    std::fs::create_dir(&target)?;
    for name in ["scripts", "reference"] {
        std::fs::create_dir(target.join(name))?;
    }
    for name in [
        "check-replay-build.cmake",
        "check-grf-language-reference.cmake",
        "check-grf-load-control-reference.cmake",
    ] {
        std::fs::copy(
            root.join("scripts").join(name),
            target.join("scripts").join(name),
        )?;
    }
    for entry in std::fs::read_dir(root.join("reference"))? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::copy(
                entry.path(),
                target.join("reference").join(entry.file_name()),
            )?;
        }
    }
    let header = target.join("reference/grf_language.hpp");
    let mut bytes = std::fs::read(&header)?;
    bytes.push(b'\n');
    std::fs::write(header, bytes)?;
    Ok(target.join("scripts/check-grf-language-reference.cmake"))
}

pub(super) struct Invocation {
    pub(super) command: Command,
    pub(super) runner: PathBuf,
    pub(super) input: String,
    pub(super) output: PathBuf,
    pub(super) diagnostic: &'static str,
    pub(super) success: bool,
}

pub(super) fn configure(
    root: &Path,
    directory: &Path,
    manifest: &mut Value,
    guard: &Guard,
) -> Result<Invocation> {
    let mut invocation = Invocation {
        command: Command::new("cmake"),
        runner: root.join("scripts/check-grf-language-reference.cmake"),
        input: root
            .join("fixtures/replay/clear-v362.sav")
            .display()
            .to_string(),
        output: directory.join("language.json"),
        diagnostic: "",
        success: false,
    };
    invocation
        .command
        .env("OTTD_GRF_LANGUAGE_OUTPUT", &invocation.output);
    match guard {
        Guard::Replay => {
            invocation
                .command
                .env("OTTD_REPLAY_PATH", directory.join("replay.json"));
            invocation.diagnostic = "refuses replay or case subset";
        }
        Guard::Subset => {
            invocation.command.env("OTTD_GRF_LANGUAGE_CASE", "english");
            invocation.diagnostic = "refuses replay or case subset";
        }
        Guard::Reused => {
            std::fs::create_dir(directory.join("native"))?;
            invocation.diagnostic = "run directory already exists";
        }
        Guard::NonSave => {
            invocation.input = "GENERATE".into();
            invocation.diagnostic = "requires an explicit saved-game load";
        }
        Guard::Stale => {
            invocation.runner = stale(root, directory)?;
            invocation.diagnostic = "Stale native replay binary/source";
        }
        Guard::MissingOutput => {
            invocation.runner = root.join("scripts/check-grf-load-control-reference.cmake");
            invocation.command.env_remove("OTTD_GRF_LANGUAGE_OUTPUT");
            invocation.diagnostic = "missing output";
        }
        Guard::DuplicateOutput => {
            invocation.runner = root.join("scripts/check-grf-load-control-reference.cmake");
            std::fs::write(&invocation.output, b"sentinel\n")?;
            invocation.diagnostic = "output exists";
        }
        Guard::Selection => {
            *manifest
                .pointer_mut("/language/selected")
                .ok_or("selection field")? = json!(255);
            invocation.diagnostic = "selected language unavailable";
        }
        Guard::Body(kind) => {
            let path = directory.join("pack-0/input.lng");
            let mut bytes = std::fs::read(&path)?;
            kind.change(&mut bytes)?;
            std::fs::write(path, bytes)?;
            invocation.diagnostic = "original selected language body rejected";
        }
        Guard::Unarmed => {
            invocation.runner = root.join("scripts/run-reference.cmake");
            invocation.input = "GENERATE".into();
            invocation.command.env_remove("OTTD_GRF_CONTROL_MANIFEST");
            invocation.success = true;
        }
    }
    Ok(invocation)
}
