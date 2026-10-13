//! Original single-locomotive after-load restoration boundary.
pub mod rail_restoration_support;
use rail_restoration_support::{Result, verify};
use std::path::PathBuf;

#[test]
#[ignore = "requires fresh original rail fixture, observation and OTTD_RAIL_ORACLE"]
fn restores_original_single_locomotive_after_load() -> Result {
    // Given an original-built, originally moved, stopped, saved and genuinely reloaded train.
    let evidence = PathBuf::from(std::env::var("OTTD_RAIL_EVIDENCE")?);
    let input = evidence.join("prepare/save/autosave/exit.sav");

    // When the isolated rail boundary restores it and both save origins continue through load.
    verify(&evidence, &input)?;

    // Then the helper retained exact full-cache, saved-world, structural and RNG comparisons.
    Ok(())
}
