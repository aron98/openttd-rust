//! Typed purchase fixtures derived from the original-created road fleet.
use ottd_save::{
    Compression, Savegame, TileRawParts, WireValue,
    world::{PathElement, World, WorldEdit},
};
use ottd_sim::content::{ContentCatalog, VehicleSpec};
use std::path::PathBuf;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn field(chunk: [u8; 4], record: u32, name: &str, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path: vec![PathElement::Field(name.into())],
        value,
    }
}
fn setting(name: &str, value: u64) -> WorldEdit {
    field(*b"PATS", 0, name, WireValue::Unsigned(value))
}
const CASES: &[(&str, u64, u64, u8)] = &[
    ("temperate-original", 0, 0, 0),
    ("temperate-realistic", 0, 1, 1),
    ("arctic", 1, 1, 2),
    ("tropic", 2, 0, 3),
    ("toyland", 3, 1, 0),
    ("limit", 0, 0, 0),
    ("money", 0, 0, 0),
    ("exact-money", 0, 0, 0),
    ("zero-cost-negative-cash", 0, 0, 0),
    ("nonowner", 0, 0, 0),
    ("unavailable", 0, 0, 0),
    ("wrong-depot", 0, 0, 0),
    ("pause", 0, 0, 0),
    ("dynamic", 0, 1, 3),
    ("legacy-paid", 0, 0, 0),
    ("errors", 0, 0, 0),
];
#[test]
#[ignore = "fresh original purchase setup: PURCHASE_SOURCE and PURCHASE_INPUT_DIR"]
fn prepare_purchase_inputs() -> Result {
    let output = PathBuf::from(std::env::var("PURCHASE_INPUT_DIR")?);
    std::fs::create_dir(&output)?;
    let base = World::decode(&Savegame::decode(
        &std::fs::read(std::env::var("PURCHASE_SOURCE")?)?,
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?;
    let (tile, depot) = base
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
        .ok_or("depot")?;
    let tile = u32::try_from(tile)?;
    let mut manifest = Vec::new();
    for &(name, climate, model, direction) in CASES {
        let mut world = base.clone();
        let mut raw = TileRawParts::from(depot);
        raw.m5 = (raw.m5 & !3) | direction;
        if name == "wrong-depot" {
            raw.m4 = 1;
        }
        let mut edits = vec![
            setting("game_creation.landscape", climate),
            setting("vehicle.roadveh_acceleration_model", model),
            setting(
                "construction.command_pause_level",
                if name == "pause" { 0 } else { 2 },
            ),
            WorldEdit::Tile {
                index: tile,
                value: raw.into(),
            },
        ];
        if climate != 0 || matches!(name, "wrong-depot" | "dynamic") {
            for id in world.tables().get(b"VEHS").ok_or("VEHS")?.records().keys() {
                edits.push(WorldEdit::RemoveRecord {
                    chunk: *b"VEHS",
                    record: *id,
                });
            }
        }
        if name == "limit" {
            edits.push(setting("vehicle.max_roadveh", 0));
        }
        if name == "money" {
            edits.push(field(*b"PLYR", 0, "money", WireValue::Signed(0)));
        }
        world.edit_batch(edits)?;
        let catalog = ContentCatalog::from_world(&world)?;
        let engines: Vec<_> = catalog
            .engines()
            .iter()
            .filter(|e| {
                matches!(e.vehicle, VehicleSpec::Road(_))
                    && u64::from(e.info.climates) & (1_u64 << climate) != 0
            })
            .collect();
        let engine = engines.first().ok_or("engine")?;
        world.edit_batch(
            engines
                .iter()
                .flat_map(|e| engine_edits(e.id, name == "dynamic"))
                .collect(),
        )?;
        configure(&mut world, name, engine)?;
        for (extension, value) in [
            ("world.json", world.saved_json()?),
            ("derived.json", serde_json::to_value(world.derived())?),
        ] {
            std::fs::write(
                output.join(format!("{name}.{extension}")),
                serde_json::to_vec(&value)?,
            )?;
        }
        std::fs::write(
            output.join(format!("{name}.sav")),
            world.to_savegame()?.encode(Compression::None)?,
        )?;
        manifest
            .push(serde_json::json!({"name":name,"tile":tile,"engine":engine.id,"cargo":engine.info.cargo_type,"engines":engines.iter().map(|e|e.id).collect::<Vec<_>>()}));
    }
    std::fs::write(output.join("manifest.json"), serde_json::to_vec(&manifest)?)?;
    Ok(())
}

fn configure(world: &mut World, name: &str, engine: &ottd_sim::content::EngineSpec) -> Result {
    let mut edits = Vec::new();
    if name == "zero-cost-negative-cash" {
        edits.extend([
            field(*b"ECMY", 0, "inflation_prices", WireValue::Unsigned(0)),
            field(*b"PLYR", 0, "money", WireValue::Signed(-1)),
            field(
                *b"PATS",
                0,
                "difficulty.infinite_money",
                WireValue::Signed(0),
            ),
        ]);
    }
    if name == "legacy-paid" {
        edits.extend(legacy_paid_edits(world)?);
    }
    if name == "exact-money" {
        let VehicleSpec::Road(spec) = engine.vehicle else {
            return Err("road engine".into());
        };
        let cost = ContentCatalog::from_world(world)?
            .price(ottd_sim::content::Price::BuildVehicleRoad)
            .saturating_mul(i64::from(spec.cost_factor))
            >> 8;
        edits.push(field(*b"PLYR", 0, "money", WireValue::Signed(cost)));
    }
    if name == "nonowner" {
        let (id, tile) = world
            .map()
            .tiles()
            .iter()
            .enumerate()
            .find(|(_, t)| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
            .ok_or("depot")?;
        let mut raw = TileRawParts::from(tile);
        raw.m1 = 1;
        edits.push(WorldEdit::Tile {
            index: u32::try_from(id)?,
            value: raw.into(),
        });
        edits.push(WorldEdit::InsertRecord {
            chunk: *b"PLYR",
            record: 1,
            value: world
                .tables()
                .get(b"PLYR")
                .and_then(|t| t.records().get(&0))
                .ok_or("company")?
                .clone(),
        });
    }
    if name == "unavailable" {
        edits.push(field(
            *b"ENGN",
            u32::from(engine.id),
            "company_avail",
            WireValue::Unsigned(0),
        ));
    }
    if name == "dynamic" {
        edits.push(depot_corner(world)?);
        for (setting, value) in [
            ("settings.vehicle.servint_roadveh", WireValue::Unsigned(42)),
            ("settings.vehicle.servint_ispercent", WireValue::Signed(1)),
        ] {
            edits.push(WorldEdit::Field {
                chunk: *b"PLYR",
                record: 0,
                path: vec![
                    PathElement::Field("settings".into()),
                    PathElement::Index(0),
                    PathElement::Field(setting.into()),
                ],
                value,
            });
        }
        edits.push(field(
            *b"DATE",
            0,
            "economy_date",
            WireValue::Signed(730_485),
        ));
    }
    world.edit_batch(edits)?;
    Ok(())
}
fn legacy_paid_edits(world: &World) -> Result<Vec<WorldEdit>> {
    Ok(world
        .tables()
        .get(b"VEHS")
        .ok_or("VEHS")?
        .records()
        .keys()
        .map(|id| WorldEdit::Field {
            chunk: *b"VEHS",
            record: *id,
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("common".into()),
                PathElement::Index(0),
                PathElement::Field("cargo_paid_for".into()),
            ],
            value: WireValue::Unsigned(37),
        })
        .collect())
}

fn depot_corner(world: &World) -> Result<WorldEdit> {
    let (id, _) = world
        .map()
        .tiles()
        .iter()
        .enumerate()
        .find(|(_, t)| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
        .ok_or("depot")?;
    let corner = id
        .checked_add(usize::try_from(world.map().width())?)
        .and_then(|v| v.checked_add(1))
        .ok_or("corner")?;
    let mut raw = TileRawParts::from(world.map().tiles().get(corner).ok_or("corner")?);
    raw.height = raw.height.checked_add(1).ok_or("height")?;
    Ok(WorldEdit::Tile {
        index: u32::try_from(corner)?,
        value: raw.into(),
    })
}

fn engine_edits(engine: u16, dynamic: bool) -> Vec<WorldEdit> {
    let mut edits = vec![field(
        *b"ENGN",
        u32::from(engine),
        "company_avail",
        WireValue::Unsigned(1),
    )];
    if dynamic {
        for (name, value) in [
            ("reliability", 1234),
            ("reliability_spd_dec", 17),
            ("flags", 2),
        ] {
            edits.push(field(
                *b"ENGN",
                u32::from(engine),
                name,
                WireValue::Unsigned(value),
            ));
        }
        edits.push(setting("vehicle.extend_vehicle_life", 5));
    }
    edits
}
