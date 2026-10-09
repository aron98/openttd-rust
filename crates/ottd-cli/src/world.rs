use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use ottd_save::{
    Compression, TileState, WireValue,
    world::{PathElement, World, WorldEdit},
};
use serde::Deserialize;
use std::{io::Write, path::Path};

/// Canonical saved-world output selection.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum View {
    /// Saved and structural state in a versioned envelope.
    #[default]
    All,
    /// Exact native saved fields.
    Saved,
    /// Content-independent structural indexes.
    Derived,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditDocument {
    schema_version: u32,
    edits: Vec<Edit>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Edit {
    Field {
        chunk: String,
        record: u32,
        path: Vec<PathElement>,
        value: EditValue,
    },
    Tile {
        index: u32,
        value: TileState,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum EditValue {
    Signed(i64),
    Unsigned(u64),
    Bytes(Vec<u8>),
    Array(Vec<Self>),
}
impl From<EditValue> for WireValue {
    fn from(value: EditValue) -> Self {
        match value {
            EditValue::Signed(value) => Self::Signed(value),
            EditValue::Unsigned(value) => Self::Unsigned(value),
            EditValue::Bytes(value) => Self::Bytes(value),
            EditValue::Array(values) => Self::Array(values.into_iter().map(Self::from).collect()),
        }
    }
}

/// Write the selected validated world view to standard output.
/// # Errors
/// Rejects unsupported saves and reports serialization or output failures.
pub fn inspect(input: &Path, limit: usize, view: View) -> Result<()> {
    let world = World::decode(&super::load(input, limit)?)?;
    let value = match view {
        View::All => {
            serde_json::json!({"schema_version":1,"saved":world.saved_json()?,"derived":world.derived()})
        }
        View::Saved => world.saved_json()?,
        View::Derived => serde_json::to_value(world.derived())?,
    };
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, &value)?;
    writeln!(output)?;
    Ok(())
}

/// Apply one bounded edit batch and publish a new save only after validation.
/// # Errors
/// Rejects malformed edits, invalid resulting worlds and existing destinations.
pub fn edit(
    input: &Path,
    edits: &Path,
    output: &Path,
    compression: Option<Compression>,
    limit: usize,
) -> Result<()> {
    ensure!(
        !output.try_exists()?,
        "destination already exists: {}",
        output.display()
    );
    let document: EditDocument =
        serde_json::from_value(super::compare::load_json(edits, limit.min(1024 * 1024))?)
            .context("invalid world edit document")?;
    ensure!(
        document.schema_version == 1,
        "unsupported world edit schema version"
    );
    ensure!(
        document.edits.len() <= 1024,
        "world edit count exceeds 1024"
    );
    for edit in &document.edits {
        match edit {
            Edit::Field { chunk, path, .. } => {
                ensure!(
                    chunk.len() == 4 && chunk.is_ascii(),
                    "chunk must contain four ASCII bytes"
                );
                ensure!(
                    !path.is_empty() && path.len() <= 64,
                    "field path must contain 1 through 64 elements"
                );
            }
            Edit::Tile { .. } => {}
        }
    }
    let save = super::load(input, limit)?;
    let compression = compression.unwrap_or_else(|| save.compression());
    let mut world = World::decode(&save)?;
    let edits = document
        .edits
        .into_iter()
        .map(|edit| -> Result<WorldEdit> {
            Ok(match edit {
                Edit::Field {
                    chunk,
                    record,
                    path,
                    value,
                } => {
                    let id: [u8; 4] = chunk
                        .as_bytes()
                        .try_into()
                        .context("chunk must contain four bytes")?;
                    WorldEdit::Field {
                        chunk: id,
                        record,
                        path,
                        value: WireValue::from(value),
                    }
                }
                Edit::Tile { index, value } => WorldEdit::Tile { index, value },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    world.edit_batch(edits).context("world edit batch failed")?;
    let bytes = world.encode(compression)?;
    ensure!(
        bytes.len() <= limit,
        "edited save exceeds {limit} byte limit"
    );
    super::publish(output, &bytes)?;
    writeln!(std::io::stdout().lock(), "{}", output.display())?;
    Ok(())
}
