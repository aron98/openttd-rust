//! Rust-only exclusivity inputs; native ordinary funding does not reach these branches.
use crate::runtime::SimulationRuntime;
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use serde_json::json;
use std::path::PathBuf;
type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires SYNTHETIC_INDUSTRY_INPUT/OUTPUT; Rust-only exclusive rights fields"]
fn synthetic_industry_and_town_exclusivity_select_actual_station_owner() -> Result {
    let input = PathBuf::from(std::env::var("SYNTHETIC_INDUSTRY_INPUT")?);
    let output = PathBuf::from(std::env::var("SYNTHETIC_INDUSTRY_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let mut rows = Vec::new();
    for (name, industry_owner, town_owner, counter, moved) in [
        ("industry-owner", 0_u8, 255_u8, 0_u8, 107_u16),
        ("industry-other", 1, 255, 0, 0),
        ("town-owner", 255, 0, 1, 107),
        ("town-other", 255, 1, 1, 0),
        ("town-expired", 255, 1, 0, 107),
    ] {
        // Given explicitly synthetic rights over the genuine first handoff input.
        let mut world = World::decode(&Savegame::decode(
            &std::fs::read(&input)?,
            ottd_save::DEFAULT_MAX_BYTES,
        )?)?;
        world.edit_batch(vec![
            field(*b"INDY", "exclusive_consumer", industry_owner),
            field(*b"CITY", "exclusivity", town_owner),
            field(*b"CITY", "exclusive_counter", counter),
        ])?;
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        let before = runtime.saved_json()?;
        // When the public loop reaches the actual forest producer and shared recipient consumer.
        runtime.advance_world(1)?;
        let after = runtime.saved_json()?;
        // Then only eligible actual station owners receive cargo; drained stock is not refunded.
        assert_eq!(
            after.pointer("/chunks/INDY/records/0/produced/0/waiting"),
            Some(&json!(13))
        );
        assert_eq!(
            after.pointer("/chunks/INDY/records/0/produced/0/history/0/transported"),
            Some(&json!(moved))
        );
        let packets = after
            .pointer("/chunks/CAPA/records")
            .ok_or("cargo packets")?;
        if moved == 0 {
            assert_eq!(packets, &json!({}));
        } else {
            assert_eq!(packets.pointer("/0/count"), Some(&json!(moved)));
            assert_eq!(packets.pointer("/0/source_id"), Some(&json!(0)));
        }
        for (label, value) in [("before", before), ("after", after)] {
            std::fs::write(
                output.join(format!("{name}-{label}.json")),
                serde_json::to_vec(&value)?,
            )?;
        }
        rows.push(
            json!({"case":name,"synthetic":true,"native_positive_claim":false,"transported":moved}),
        );
    }
    std::fs::write(output.join("PASS.json"), serde_json::to_vec_pretty(&rows)?)?;
    Ok(())
}

fn field(chunk: [u8; 4], name: &str, value: u8) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record: 0,
        path: vec![PathElement::Field(name.into())],
        value: WireValue::Unsigned(u64::from(value)),
    }
}
