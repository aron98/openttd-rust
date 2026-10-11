//! Owner, configured-content and nonempty cargo refusal at the complete-consist boundary.
pub mod rail_consist_support;
use ottd_save::{
    TableRecord, TileRawParts, WireValue,
    world::{PathElement, WorldEdit},
};
use ottd_sim::runtime::rail::{self, ConsistError, ConsistRefusal};
use rail_consist_support::{Result, load, save};
use std::path::PathBuf;

fn common(name: &str) -> Vec<PathElement> {
    vec![
        PathElement::Field("train".into()),
        PathElement::Index(0),
        PathElement::Field("common".into()),
        PathElement::Index(0),
        PathElement::Field(name.into()),
    ]
}

#[test]
#[ignore = "requires authentic vanilla consist; never invokes native on invalid context"]
fn rejects_foreign_track_owner_and_newgrf_context() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?);
    let original = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let directory = root.join("context-refusals");
    std::fs::create_dir(&directory)?;
    let company = original
        .tables()
        .get(b"PLYR")
        .and_then(|t| t.records().get(&0))
        .ok_or("company")?
        .clone();
    let mut owner_world = original.clone();
    let saved = original.saved_json()?;
    let tile_id = u32::try_from(
        saved
            .pointer("/chunks/VEHS/records/1/train/0/common/0/tile")
            .and_then(serde_json::Value::as_u64)
            .ok_or("wagon tile")?,
    )?;
    let mut track = TileRawParts::from(
        original
            .map()
            .tiles()
            .get(usize::try_from(tile_id)?)
            .ok_or("track")?,
    );
    track.m1 = (track.m1 & !31) | 1;
    owner_world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"PLYR",
            record: 1,
            value: company,
        },
        WorldEdit::Tile {
            index: tile_id,
            value: track.into(),
        },
    ])?;
    let ngrf = original.tables().get(b"NGRF").ok_or("NGRF")?;
    let values = ngrf
        .schema()
        .fields()
        .iter()
        .map(|field| match field.name() {
            "filename" => Ok(WireValue::Bytes(b"unsupported.grf".to_vec())),
            "ident.grfid" => Ok(WireValue::Unsigned(1)),
            "ident.md5sum" => Ok(WireValue::Array(vec![WireValue::Unsigned(0); 16])),
            "param" => Ok(WireValue::Array(vec![WireValue::Unsigned(0); 128])),
            "num_params" | "version" | "palette" => Ok(WireValue::Unsigned(0)),
            _ => Err("unexpected NGRF schema"),
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut configuration = ngrf.clone();
    configuration
        .records_mut()
        .insert(0, TableRecord::new(values));
    let mut configured_save = original.to_savegame()?;
    configured_save.replace_chunk(configuration.encode()?)?;
    let content_world = ottd_save::world::World::decode(&configured_save)?;
    for (name, mut world) in [("owner", owner_world), ("content", content_world)] {
        let before = world.to_savegame()?.encode(ottd_save::Compression::None)?;
        let derived = serde_json::to_value(world.derived())?;
        save(&world, &directory.join(format!("{name}.before.sav")))?;
        let result = rail::restore_consists(&mut world);
        match name {
            "owner" => assert!(
                matches!(
                    result,
                    Err(ConsistError::Part {
                        reason: ConsistRefusal::Owner,
                        staged_parts: 1,
                        ..
                    })
                ),
                "{result:?}"
            ),
            "content" => assert!(
                matches!(result, Err(ConsistError::Content(_))),
                "{result:?}"
            ),
            _ => return Err("unknown context case".into()),
        }
        assert_eq!(
            world.to_savegame()?.encode(ottd_save::Compression::None)?,
            before
        );
        assert_eq!(serde_json::to_value(world.derived())?, derived);
        save(&world, &directory.join(format!("{name}.after.sav")))?;
        std::fs::write(
            directory.join(format!("{name}.result.txt")),
            format!("PASS {result:?}; whole world unchanged; native not invoked\n"),
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "requires authentic vanilla consist; reserved and stored cargo stay unsupported"]
fn rejects_nonempty_cargo_without_partial_publication() -> Result {
    let root = PathBuf::from(std::env::var("OTTD_CONSIST_EVIDENCE")?);
    let mut world = load(&root.join("prepare/save/autosave/exit.sav"))?;
    let table = world.tables().get(b"CAPA").ok_or("CAPA")?;
    let values = table
        .schema()
        .fields()
        .iter()
        .map(|field| match field.name() {
            "count" => Ok(WireValue::Unsigned(1)),
            "source" | "source_id" => Ok(WireValue::Unsigned(65535)),
            "source_xy" | "loaded_at_xy" | "periods_in_transit" | "source_type" => {
                Ok(WireValue::Unsigned(0))
            }
            "feeder_share" | "travelled.x" | "travelled.y" => Ok(WireValue::Signed(0)),
            _ => Err("unexpected cargo schema"),
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    world.edit_batch(vec![
        WorldEdit::InsertRecord {
            chunk: *b"CAPA",
            record: 0,
            value: TableRecord::new(values),
        },
        WorldEdit::ArrayList {
            chunk: *b"VEHS",
            record: 1,
            path: common("cargo.packets"),
            values: vec![WireValue::Unsigned(1)],
        },
        WorldEdit::Field {
            chunk: *b"VEHS",
            record: 1,
            path: common("cargo.action_counts"),
            value: WireValue::Array(vec![
                WireValue::Unsigned(1),
                WireValue::Unsigned(0),
                WireValue::Unsigned(0),
                WireValue::Unsigned(0),
            ]),
        },
    ])?;
    let before = world.to_savegame()?.encode(ottd_save::Compression::None)?;
    let derived = serde_json::to_value(world.derived())?;
    save(&world, &root.join("cargo.before.sav"))?;
    let result = rail::restore_consists(&mut world);
    assert!(
        matches!(
            result,
            Err(ConsistError::Unsupported(ConsistRefusal::Cargo))
        ),
        "{result:?}"
    );
    assert_eq!(
        world.to_savegame()?.encode(ottd_save::Compression::None)?,
        before
    );
    assert_eq!(serde_json::to_value(world.derived())?, derived);
    save(&world, &root.join("cargo.after.sav"))?;
    std::fs::write(
        root.join("cargo-refusal.txt"),
        format!("PASS {result:?}; native not invoked\n"),
    )?;
    Ok(())
}
