use super::*;

#[test]
#[ignore = "SALE_EMPTY, SALE_SEED, SALE_INPUTS: original cleared fleet and fresh purchase"]
fn prepare_sale_inputs() -> Result {
    let output = PathBuf::from(std::env::var("SALE_INPUTS")?);
    std::fs::create_dir(&output)?;
    let empty = read(Path::new(&std::env::var("SALE_EMPTY")?))?;
    let seed = read(Path::new(&std::env::var("SALE_SEED")?))?;
    let pair = read(Path::new(&std::env::var("SALE_PAIR")?))?;
    let tile = u32::try_from(
        empty
            .map()
            .tiles()
            .iter()
            .position(|t| t.tile_type() >> 4 == 2 && t.m5() >> 6 == 2)
            .ok_or("depot")?,
    )?;
    let mut manifest = Vec::new();
    for (name, climate, model, seeded) in [
        ("temperate-original", 0, 0, false),
        ("temperate-realistic", 0, 1, false),
        ("arctic", 1, 1, false),
        ("tropic", 2, 0, false),
        ("toyland", 3, 1, false),
        ("reuse", 0, 0, false),
        ("named-survivor", 0, 0, true),
        ("legacy", 0, 0, true),
        ("crashed", 0, 0, true),
        ("moving", 0, 0, true),
        ("unstopped", 0, 0, true),
        ("wrong-state", 0, 0, true),
        ("outside-depot", 0, 0, true),
        ("nonowner", 0, 0, true),
        ("pause", 0, 0, true),
        ("negative-value", 0, 0, true),
        ("minimum-value", 0, 0, true),
        ("zero-value", 0, 0, true),
        ("insufficient", 0, 0, true),
        ("missing", 0, 0, true),
        ("zero-location", 0, 0, true),
        ("invalid-location", 0, 0, true),
    ] {
        let mut world = if name == "named-survivor" {
            pair.clone()
        } else if seeded {
            seed.clone()
        } else {
            empty.clone()
        };
        world.edit_batch(vec![
            field(
                *b"PATS",
                "game_creation.landscape",
                WireValue::Unsigned(climate),
            ),
            field(
                *b"PATS",
                "vehicle.roadveh_acceleration_model",
                WireValue::Unsigned(model),
            ),
            field(
                *b"PATS",
                "construction.command_pause_level",
                WireValue::Unsigned(if name == "pause" { 0 } else { 2 }),
            ),
        ])?;
        let catalog = ContentCatalog::from_world(&world)?;
        let engines: Vec<_> = catalog
            .engines()
            .iter()
            .filter(|e| {
                matches!(e.vehicle, VehicleSpec::Road(_))
                    && u64::from(e.info.climates) & (1_u64 << climate) != 0
            })
            .map(|e| e.id)
            .collect();
        let mut edits: Vec<_> = engines
            .iter()
            .map(|id| WorldEdit::Field {
                chunk: *b"ENGN",
                record: u32::from(*id),
                path: vec![PathElement::Field("company_avail".into())],
                value: WireValue::Unsigned(1),
            })
            .collect();
        configure(&mut edits, name, &world)?;
        world.edit_batch(edits)?;
        write(&world, &output, name)?;
        manifest.push(serde_json::json!({"name":name,"tile":tile,"engines":engines}));
    }
    std::fs::write(output.join("manifest.json"), serde_json::to_vec(&manifest)?)?;
    Ok(())
}
fn configure(edits: &mut Vec<WorldEdit>, name: &str, world: &World) -> Result {
    match name {
        "outside-depot" => outside_depot(edits, world)?,
        "named-survivor" => {
            edits.push(named_group(world)?);
            edits.push(vehicle(1, "group_id", WireValue::Unsigned(0)));
        }
        "legacy" => edits.push(vehicle(0, "cargo_paid_for", WireValue::Unsigned(37))),
        "crashed" => {
            edits.push(vehicle(0, "vehstatus", WireValue::Unsigned(128)));
            edits.push(vehicle(0, "cur_speed", WireValue::Unsigned(1)));
        }
        "moving" => edits.push(vehicle(0, "cur_speed", WireValue::Unsigned(1))),
        "unstopped" => edits.push(vehicle(0, "vehstatus", WireValue::Unsigned(9))),
        "wrong-state" => edits.push(WorldEdit::Field {
            chunk: *b"VEHS",
            record: 0,
            path: vec![
                PathElement::Field("roadveh".into()),
                PathElement::Index(0),
                PathElement::Field("state".into()),
            ],
            value: WireValue::Unsigned(0),
        }),
        "nonowner" => {
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
            edits.push(vehicle(0, "owner", WireValue::Unsigned(1)));
        }
        "negative-value" => edits.push(vehicle(0, "value", WireValue::Signed(-9))),
        "minimum-value" => {
            edits.push(vehicle(0, "value", WireValue::Signed(i64::MIN)));
            edits.push(field(*b"PLYR", "money", WireValue::Signed(i64::MAX)));
        }
        "zero-value" => {
            edits.push(vehicle(0, "value", WireValue::Signed(0)));
            edits.push(field(*b"PLYR", "money", WireValue::Signed(-1)));
        }
        "insufficient" => {
            edits.push(vehicle(0, "value", WireValue::Signed(-9)));
            edits.push(field(*b"PLYR", "money", WireValue::Signed(0)));
        }
        "temperate-original"
        | "temperate-realistic"
        | "arctic"
        | "tropic"
        | "toyland"
        | "reuse"
        | "pause"
        | "missing"
        | "zero-location"
        | "invalid-location" => {}
        _ => return Err("unknown sale fixture".into()),
    }
    Ok(())
}

fn outside_depot(edits: &mut Vec<WorldEdit>, world: &World) -> Result {
    let tile = u32::try_from(
        world
            .map()
            .tiles()
            .iter()
            .position(|tile| tile.tile_type() >> 4 == 2 && tile.m5() >> 6 == 0)
            .ok_or("ordinary road")?,
    )?;
    let width = world.map().width();
    let height = world
        .map()
        .tiles()
        .get(usize::try_from(tile)?)
        .ok_or("tile")?
        .height();
    for (name, value) in [
        ("tile", WireValue::Unsigned(u64::from(tile))),
        (
            "x_pos",
            WireValue::Unsigned(u64::from(
                tile.checked_rem(width)
                    .ok_or("map width")?
                    .saturating_mul(16)
                    .saturating_add(8),
            )),
        ),
        (
            "y_pos",
            WireValue::Unsigned(u64::from(
                tile.checked_div(width)
                    .ok_or("map width")?
                    .saturating_mul(16)
                    .saturating_add(8),
            )),
        ),
        (
            "z_pos",
            WireValue::Signed(i64::from(height).saturating_mul(8)),
        ),
    ] {
        edits.push(vehicle(0, name, value));
    }
    Ok(())
}

fn named_group(world: &World) -> Result<WorldEdit> {
    let table = world.tables().get(b"GRPS").ok_or("GRPS")?;
    let values = table
        .schema()
        .fields()
        .iter()
        .map(|field| -> Result<WireValue> {
            Ok(match field.name() {
                "name" => WireValue::Bytes(b"Survivor".to_vec()),
                "vehicle_type" | "number" => WireValue::Unsigned(1),
                "owner" | "flags" | "livery.in_use" | "livery.colour1" | "livery.colour2" => {
                    WireValue::Unsigned(0)
                }
                "parent" => WireValue::Unsigned(65535),
                _ => return Err("unknown group field".into()),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(WorldEdit::InsertRecord {
        chunk: *b"GRPS",
        record: 0,
        value: ottd_save::TableRecord::new(values),
    })
}
