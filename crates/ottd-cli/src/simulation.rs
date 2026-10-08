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
pub(crate) fn run(input: &Path, limit: usize, ticks: u32) -> Result<()> {
    ensure!(
        ticks <= ottd_sim::MAX_TICKS,
        "requested ticks exceed the per-request limit of {}",
        ottd_sim::MAX_TICKS
    );
    let mut bytes = Vec::new();
    File::open(input)
        .with_context(|| format!("cannot open {}", input.display()))?
        .take(u64::try_from(limit)?.saturating_add(1))
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "simulation JSON exceeds --max-bytes");
    let fixture: ottd_sim::Fixture =
        serde_json::from_slice(&bytes).context("invalid simulation fixture JSON")?;
    let output = ottd_sim::simulate(fixture, ticks)?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &output)?;
    writeln!(stdout)?;
    Ok(())
}
