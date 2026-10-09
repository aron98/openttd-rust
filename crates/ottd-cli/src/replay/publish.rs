use anyhow::{Context, Result};
use std::{fs, path::Path};
pub(super) fn directory(stage: &Path, output: &Path) -> Result<()> {
    let mut entries = fs::read_dir(stage)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| match entry.file_name().to_str() {
        Some("checkpoint.json") => 1,
        Some("results.json") => 2,
        _ => 0,
    });
    fs::create_dir(output)
        .with_context(|| format!("cannot reserve new replay output {}", output.display()))?;
    for entry in entries {
        fs::hard_link(entry.path(),output.join(entry.file_name())).with_context(||format!("cannot publish replay artifact {}; {} may contain partial validated artifacts; results.json is published last",entry.file_name().to_string_lossy(),output.display()))?;
    }
    Ok(())
}
