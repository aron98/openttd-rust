mod envelope;
mod observe;
mod publish;
use anyhow::{Context, Result, ensure};
use ottd_save::world::World;
use ottd_sim::{ReplayCursor, ReplayError, ReplayPlan, run_replay};
use std::{io::Write, path::Path};

/// Execute a bounded replay and publish a new output directory.
/// # Errors
/// Rejects invalid inputs, unsupported worlds and existing destinations.
pub fn start(
    input: &Path,
    actions: &Path,
    output: &Path,
    through: Option<u64>,
    limit: usize,
) -> Result<()> {
    ensure!(
        !output.try_exists()?,
        "destination already exists: {}",
        output.display()
    );
    let plan: ReplayPlan = serde_json::from_value(super::compare::load_json(
        actions,
        limit.min(16 * 1024 * 1024),
    )?)
    .context("invalid replay plan")?;
    let cursor = ReplayCursor::new(plan)?;
    let world = World::decode(&super::load(input, limit)?)?;
    execute(&world, &cursor, output, through, limit)
}
/// Resume a save-identity-verified checkpoint into a new output directory.
/// # Errors
/// Rejects altered saves, invalid cursor state and replay or publication failures.
pub fn resume(checkpoint: &Path, output: &Path, through: Option<u64>, limit: usize) -> Result<()> {
    ensure!(
        !output.try_exists()?,
        "destination already exists: {}",
        output.display()
    );
    let (world, cursor) = envelope::load(checkpoint, limit)?;
    execute(&world, &cursor, output, through, limit)
}
fn execute(
    world: &World,
    cursor: &ReplayCursor,
    output: &Path,
    through: Option<u64>,
    limit: usize,
) -> Result<()> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut stage = observe::Stage::new(parent, limit)?;
    let outcome = run_replay(world, cursor, through, &mut |event, world| {
        stage
            .observe(event, world)
            .map_err(|e| ReplayError::Observer(e.to_string()))
    })?;
    stage.finish(outcome.cursor)?;
    publish::directory(stage.path(), output)?;
    writeln!(std::io::stdout().lock(), "{}", output.display())?;
    Ok(())
}
