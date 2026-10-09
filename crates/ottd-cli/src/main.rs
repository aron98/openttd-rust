//! Save compatibility tools for the OpenTTD Rust port.

use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use ottd_save::{Compression, DEFAULT_MAX_BYTES, Savegame};
use serde_json::json;

mod callbacks;
/// Exact JSON comparison helpers.
pub mod compare;
mod simulation;
/// Saved-world command adapters.
pub mod world;

#[derive(Debug, Parser)]
#[command(
    name = "ottd",
    version,
    about = "OpenTTD 15.3 save compatibility tools (not yet a game)"
)]
struct Cli {
    #[arg(long, global = true, default_value_t = DEFAULT_MAX_BYTES)]
    max_bytes: usize,
    #[command(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    #[command(about = "Export version-362 saved state and structural indexes; not gameplay caches")]
    World {
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = world::View::All)]
        view: world::View,
    },
    #[command(about = "Apply a bounded JSON saved-state edit batch; destination must not exist")]
    EditWorld {
        input: PathBuf,
        edits: PathBuf,
        output: PathBuf,
        #[arg(long, value_enum)]
        compression: Option<OutputCompression>,
    },
    #[command(
        about = "Run one supported object callback on explicit JSON state; not a full game tick"
    )]
    SimulateCallbacks { input: PathBuf },
    #[command(about = "Advance isolated temperate clear-landscape JSON; not a full game or save")]
    SimulateLandscape {
        input: PathBuf,
        #[arg(long)]
        ticks: u32,
    },
    #[command(about = "Decode a version-362 save into a typed world snapshot as JSON")]
    Snapshot { input: PathBuf },
    #[command(about = "Compare every JSON field and report the first differing path")]
    Compare { expected: PathBuf, actual: PathBuf },
    #[command(
        about = "Inspect save version and chunk framing as JSON; does not validate game objects"
    )]
    Inspect { input: PathBuf },
    #[command(about = "Losslessly rewrite a save; destination must not exist")]
    Rewrite {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, value_enum)]
        compression: Option<OutputCompression>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputCompression {
    None,
    Zlib,
    Lzma,
    Lzo,
}

impl From<OutputCompression> for Compression {
    fn from(value: OutputCompression) -> Self {
        match value {
            OutputCompression::None => Self::None,
            OutputCompression::Zlib => Self::Zlib,
            OutputCompression::Lzma => Self::Lzma,
            OutputCompression::Lzo => Self::Lzo,
        }
    }
}

fn load(path: &Path, limit: usize) -> Result<Savegame> {
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut input = Vec::new();
    let bounded = u64::try_from(limit)?.saturating_add(1);
    file.take(bounded).read_to_end(&mut input)?;
    Savegame::decode(&input, limit).with_context(|| format!("invalid save: {}", path.display()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Action::World { input, view } => world::inspect(&input, cli.max_bytes, view)?,
        Action::EditWorld {
            input,
            edits,
            output,
            compression,
        } => world::edit(
            &input,
            &edits,
            &output,
            compression.map(Compression::from),
            cli.max_bytes,
        )?,
        Action::SimulateCallbacks { input } => callbacks::run(&input, cli.max_bytes)?,
        Action::SimulateLandscape { input, ticks } => {
            simulation::run(&input, cli.max_bytes, ticks)?;
        }
        Action::Snapshot { input } => {
            let snapshot = load(&input, cli.max_bytes)?.snapshot()?;
            let mut output = std::io::stdout().lock();
            serde_json::to_writer(&mut output, &snapshot)?;
            writeln!(output)?;
        }
        Action::Compare { expected, actual } => {
            let expected = compare::load_json(&expected, cli.max_bytes)?;
            let actual = compare::load_json(&actual, cli.max_bytes)?;
            if let Some(difference) = compare::first_difference(&expected, &actual, "$") {
                anyhow::bail!("{difference}");
            }
            writeln!(std::io::stdout().lock(), "snapshots match")?;
        }
        Action::Inspect { input } => {
            let save = load(&input, cli.max_bytes)?;
            let chunks: Vec<_> = save
                .chunks()
                .iter()
                .map(|chunk| {
                    json!({
                        "id": String::from_utf8_lossy(&chunk.id()),
                        "kind": format!("{:?}", chunk.kind()),
                        "body_bytes": chunk.body().len(),
                        "framed_records": chunk.records(),
                    })
                })
                .collect();
            let report = json!({
                "savegame_version": save.version(),
                "compression": format!("{:?}", save.compression()),
                "validation": "container-only",
                "chunks": chunks,
            });
            let mut output = std::io::stdout().lock();
            serde_json::to_writer_pretty(&mut output, &report)?;
            writeln!(output)?;
        }
        Action::Rewrite {
            input,
            output,
            compression,
        } => {
            ensure!(
                !output.try_exists()?,
                "destination already exists: {}",
                output.display()
            );
            let save = load(&input, cli.max_bytes)?;
            let format = compression.map_or_else(|| save.compression(), Compression::from);
            let bytes = save.encode(format)?;
            publish(&output, &bytes)?;
            writeln!(std::io::stdout().lock(), "{}", output.display())?;
        }
    }
    Ok(())
}

fn publish(output: &Path, bytes: &[u8]) -> Result<()> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(output)
        .with_context(|| format!("cannot create {}", output.display()))?;
    Ok(())
}
