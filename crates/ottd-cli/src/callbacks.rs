use anyhow::{Context, Result, ensure};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

#[expect(
    clippy::redundant_pub_crate,
    reason = "crate-private adapter is intentionally not part of the public API"
)]
pub(crate) fn run(input: &Path, limit: usize) -> Result<()> {
    let mut bytes = Vec::new();
    File::open(input)
        .with_context(|| format!("cannot open {}", input.display()))?
        .take(u64::try_from(limit)?.saturating_add(1))
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "callback JSON exceeds --max-bytes");
    let fixture: ottd_sim::CallbackFixture =
        serde_json::from_slice(&bytes).context("invalid callback fixture JSON")?;
    let output = ottd_sim::simulate_callback(fixture)?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &output)?;
    writeln!(stdout)?;
    Ok(())
}
