//! Explicitly synthetic arithmetic inputs exercise the actual forest/public cargo transaction.
use crate::runtime::SimulationRuntime;
use ottd_save::{
    Savegame, WireValue,
    world::{PathElement, World, WorldEdit},
};
use serde_json::json;
use std::path::PathBuf;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "requires SYNTHETIC_INDUSTRY_INPUT/OUTPUT; altered Rust-only history/stock/rating inputs"]
fn synthetic_forest_stock_history_wrapping_and_fraction_only_handoff() -> Result {
    let input = PathBuf::from(std::env::var("SYNTHETIC_INDUSTRY_INPUT")?);
    let output = PathBuf::from(std::env::var("SYNTHETIC_INDUSTRY_OUTPUT")?);
    std::fs::create_dir_all(&output)?;
    let mut cases = Vec::new();
    for (name, waiting, history, rating, moved, fraction) in [
        ("history-wrap", 156_u16, 65_500_u16, 175_u8, 107_u16, 64_u8),
        ("clamped-stock", 65_535, 65_500, 175, 175, 80),
        ("fraction-only", 31, 0, 1, 0, 62),
        ("zero-rating", 156, 0, 0, 0, 0),
    ] {
        // Given explicit synthetic saved arithmetic values over the real first handoff input.
        let mut world = World::decode(&Savegame::decode(
            &std::fs::read(&input)?,
            ottd_save::DEFAULT_MAX_BYTES,
        )?)?;
        let mut edits = vec![industry(&["waiting"], u64::from(waiting))];
        for field in ["production", "transported"] {
            edits.push(industry(&["history", field], u64::from(history)));
        }
        edits.push(WorldEdit::Field {
            chunk: *b"STNN",
            record: 0,
            path: vec![
                PathElement::Field("normal".into()),
                PathElement::Index(0),
                PathElement::Field("goods".into()),
                PathElement::Index(7),
                PathElement::Field("rating".into()),
            ],
            value: WireValue::Unsigned(u64::from(rating)),
        });
        world.edit_batch(edits)?;
        let mut runtime = SimulationRuntime::restore_vanilla(world)?;
        let before = runtime.saved_json()?;
        // When the public world loop reaches the unchanged first forest callback.
        runtime.advance_world(1)?;
        // Then clamped stock, wrapping history and actual whole admitted cargo agree.
        let after = runtime.saved_json()?;
        let produced = after
            .pointer("/chunks/INDY/records/0/produced/0")
            .ok_or("produced slot")?;
        let drained = waiting.min(255);
        // The later stock tick also fires on this genuine callback call (counter 65280).
        assert_eq!(produced["waiting"], (waiting - drained).saturating_add(13));
        assert_eq!(
            produced.pointer("/history/0/production"),
            Some(&json!(history.wrapping_add(drained)))
        );
        assert_eq!(
            produced.pointer("/history/0/transported"),
            Some(&json!(history.wrapping_add(moved)))
        );
        assert_eq!(
            after.pointer("/chunks/STNN/records/0/normal/0/goods/7/amount_fract"),
            Some(&json!(fraction)),
        );
        let packets = after.pointer("/chunks/CAPA/records").ok_or("packets")?;
        if moved == 0 {
            assert_eq!(packets, &json!({}));
        } else {
            assert_eq!(packets.pointer("/0/count"), Some(&json!(moved)));
            assert_eq!(packets.pointer("/0/source_type"), Some(&json!(0)));
            assert_eq!(packets.pointer("/0/source_id"), Some(&json!(0)));
        }
        for (label, value) in [("before", before), ("after", after)] {
            std::fs::write(
                output.join(format!("{name}-{label}.json")),
                serde_json::to_vec(&value)?,
            )?;
        }
        cases.push(
            json!({"case":name,"synthetic":true,"stock":waiting,"history":history,
            "rating":rating,"clamped":drained,"actual_whole_moved":moved,"fraction":fraction}),
        );
    }
    std::fs::write(output.join("PASS.json"), serde_json::to_vec_pretty(&cases)?)?;
    Ok(())
}

fn industry(fields: &[&str], value: u64) -> WorldEdit {
    let mut path = vec![PathElement::Field("produced".into()), PathElement::Index(0)];
    for field in fields {
        path.push(PathElement::Field((*field).into()));
        if *field == "history" {
            path.push(PathElement::Index(0));
        }
    }
    WorldEdit::Field {
        chunk: *b"INDY",
        record: 0,
        path,
        value: WireValue::Unsigned(value),
    }
}
