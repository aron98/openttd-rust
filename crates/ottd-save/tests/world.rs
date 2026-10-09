//! Complete-world loading, edits and structural validation regressions.
#![cfg(test)]
use ottd_save::{
    Compression, Savegame, WireValue,
    world::{PathElement, World, WorldEdit, WorldError},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn populated() -> Result<World> {
    Ok(World::decode(&Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?)?)
}
fn field(name: &str) -> PathElement {
    PathElement::Field(name.to_owned())
}
fn vehicle(kind: &str, name: &str) -> Vec<PathElement> {
    vec![
        field(kind),
        PathElement::Index(0),
        field("common"),
        PathElement::Index(0),
        field(name),
    ]
}
fn edit(chunk: [u8; 4], record: u32, path: Vec<PathElement>, value: WireValue) -> WorldEdit {
    WorldEdit::Field {
        chunk,
        record,
        path,
        value,
    }
}

#[test]
fn restores_generated_world_when_all_required_chunks_are_present() -> Result {
    let save = Savegame::decode(
        include_bytes!("../../../fixtures/generated-v362.sav"),
        4 * 1024 * 1024,
    )?;
    let world = World::decode(&save)?;
    assert_eq!(world.map().tiles().len(), 4096);
    Ok(())
}

#[test]
fn restores_vehicle_chains_and_cargo_from_populated_world() -> Result {
    let world = populated()?;
    let chain = world
        .derived()
        .vehicles
        .iter()
        .find(|v| v.id == 19)
        .ok_or("missing vehicle")?;
    assert_eq!((chain.previous, chain.first), (Some(18), 17));
    let cargo = world
        .derived()
        .cargo_lists
        .iter()
        .find(|c| c.owner.id == 21 && c.cargo_type.is_none())
        .ok_or("missing cargo")?;
    assert_eq!(
        (&cargo.packets, cargo.count, cargo.periods_in_transit),
        (&vec![3, 2], 24, 68)
    );
    Ok(())
}

#[test]
fn rejects_invalid_reference_and_keeps_world_unchanged() -> Result {
    let mut world = populated()?;
    let before = world.saved_json()?;
    let error = world.edit_field(
        *b"VEHS",
        12,
        &vehicle("roadveh", "next"),
        WireValue::Unsigned(999_999),
    );
    assert!(matches!(
        error,
        Err(WorldError::Invalid {
            reason: "dangling object reference",
            ..
        })
    ));
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn rejects_vehicle_cycle_and_multiple_predecessors() -> Result {
    for (id, target) in [(19, 18), (20, 19)] {
        let mut world = populated()?;
        let error = world.edit_field(
            *b"VEHS",
            id,
            &vehicle("train", "next"),
            WireValue::Unsigned(target),
        );
        assert!(error.is_err());
    }
    Ok(())
}

#[test]
fn rejects_cross_type_consist_links() -> Result {
    let mut world = populated()?;
    let result = world.edit_field(
        *b"VEHS",
        12,
        &vehicle("roadveh", "next"),
        WireValue::Unsigned(17),
    );
    assert!(result.is_err());
    Ok(())
}

#[test]
fn rejects_inconsistent_cargo_action_counts() -> Result {
    let mut world = populated()?;
    let result = world.edit_field(*b"CAPA", 3, &[field("count")], WireValue::Unsigned(21));
    assert!(result.is_err());
    Ok(())
}

#[test]
fn applies_coupled_shared_order_edits_in_one_transaction() -> Result {
    let mut world = populated()?;
    world.edit_batch(vec![
        edit(
            *b"VEHS",
            12,
            vehicle("roadveh", "next_shared"),
            WireValue::Unsigned(14),
        ),
        edit(
            *b"VEHS",
            13,
            vehicle("roadveh", "orders"),
            WireValue::Unsigned(1),
        ),
    ])?;
    let list = world
        .derived()
        .order_lists
        .iter()
        .find(|l| l.id == 0)
        .ok_or("missing list")?;
    assert_eq!(
        (&list.vehicles, list.first_shared),
        (&vec![12, 13], Some(12))
    );
    let bytes = world.encode(Compression::Zlib)?;
    let reloaded = World::decode(&Savegame::decode(&bytes, ottd_save::DEFAULT_MAX_BYTES)?)?;
    assert_eq!(world.derived(), reloaded.derived());
    Ok(())
}

#[test]
fn rolls_back_entire_batch_when_last_edit_fails() -> Result {
    let mut world = populated()?;
    let before = world.saved_json()?;
    let result = world.edit_batch(vec![
        edit(*b"PLYR", 1, vec![field("money")], WireValue::Signed(1234)),
        edit(
            *b"VEHS",
            12,
            vehicle("roadveh", "next"),
            WireValue::Unsigned(999_999),
        ),
    ]);
    assert!(result.is_err());
    assert_eq!(world.saved_json()?, before);
    Ok(())
}

#[test]
fn rejects_duplicate_cargo_ownership() -> Result {
    let mut world = populated()?;
    let path = vec![
        field("normal"),
        PathElement::Index(0),
        field("goods"),
        PathElement::Index(0),
        field("cargo"),
        PathElement::Index(0),
        field("second"),
        PathElement::Index(0),
    ];
    let json = world.saved_json()?;
    let station = json
        .pointer("/chunks/STNN/records")
        .ok_or("station records")?
        .as_object()
        .ok_or("station records")?
        .iter()
        .find(|(_, r)| {
            r.pointer("/normal/0/goods/0/cargo/0/second/0")
                .is_some_and(serde_json::Value::is_number)
        })
        .map(|(id, _)| id.parse::<u32>())
        .transpose()?
        .ok_or("waiting cargo station")?;
    let result = world.edit_field(*b"STNN", station, &path, WireValue::Unsigned(4));
    assert!(matches!(
        result,
        Err(WorldError::Invalid {
            reason: "packet belongs to multiple lists",
            ..
        })
    ));
    Ok(())
}

#[test]
fn preserves_deleted_cargo_source_ids() -> Result {
    let mut world = populated()?;
    world.edit_field(*b"CAPA", 3, &[field("source")], WireValue::Unsigned(63_999))?;
    assert_eq!(
        world.saved_json()?.pointer("/chunks/CAPA/records/3/source"),
        Some(&serde_json::json!(63_999))
    );
    Ok(())
}

#[test]
fn recalculates_order_duration_after_saved_edit() -> Result {
    let mut world = populated()?;
    world.edit_batch(vec![
        edit(
            *b"ORDL",
            0,
            vec![field("orders"), PathElement::Index(0), field("wait_time")],
            WireValue::Unsigned(111),
        ),
        edit(
            *b"ORDL",
            0,
            vec![field("orders"), PathElement::Index(0), field("flags")],
            WireValue::Unsigned(40),
        ),
    ])?;
    let list = world
        .derived()
        .order_lists
        .iter()
        .find(|l| l.id == 0)
        .ok_or("missing list")?;
    assert_eq!(
        (
            list.num_manual_orders,
            list.total_duration,
            list.timetable_duration
        ),
        (2, 111, 111)
    );
    Ok(())
}

#[test]
fn rejects_group_cycle() -> Result {
    let mut world = populated()?;
    let result = world.edit_field(*b"GRPS", 0, &[field("parent")], WireValue::Unsigned(0));
    assert!(matches!(
        result,
        Err(WorldError::Invalid {
            reason: "cyclic group hierarchy",
            ..
        })
    ));
    Ok(())
}

#[test]
fn rejects_fixed_array_resize_and_invalid_boolean() -> Result {
    let mut world = populated()?;
    assert!(
        world
            .edit_field(
                *b"PLYR",
                1,
                &[field("yearly_expenses")],
                WireValue::Array(Vec::new())
            )
            .is_err()
    );
    assert!(
        world
            .edit_field(*b"PLYR", 1, &[field("is_ai")], WireValue::Signed(2))
            .is_err()
    );
    Ok(())
}

#[test]
fn preserves_container_version_bytes() -> Result {
    let world = populated()?;
    let mut encoded = world.encode(Compression::None)?;
    *encoded.get_mut(6).ok_or("header")? = 3;
    *encoded.get_mut(7).ok_or("header")? = 9;
    let world = World::decode(&Savegame::decode(&encoded, ottd_save::DEFAULT_MAX_BYTES)?)?;
    assert_eq!(
        world.encode(Compression::None)?.get(4..8),
        Some([1, 106, 3, 9].as_slice())
    );
    Ok(())
}

#[test]
fn saturates_vehicle_feeder_share_like_native_money() -> Result {
    let mut world = populated()?;
    world.edit_batch(vec![
        edit(
            *b"CAPA",
            3,
            vec![field("feeder_share")],
            WireValue::Signed(i64::MAX),
        ),
        edit(
            *b"CAPA",
            2,
            vec![field("feeder_share")],
            WireValue::Signed(1),
        ),
    ])?;
    let cargo = world
        .derived()
        .cargo_lists
        .iter()
        .find(|c| c.owner.id == 21 && c.cargo_type.is_none())
        .ok_or("missing cargo")?;
    assert_eq!(cargo.feeder_share, i64::MAX);
    Ok(())
}

#[test]
fn rejects_active_ai_without_running_script_tail() -> Result {
    let mut world = populated()?;
    assert!(
        world
            .edit_field(*b"PLYR", 0, &[field("is_ai")], WireValue::Signed(1))
            .is_err()
    );
    Ok(())
}

#[test]
fn rejects_inactive_ai_with_running_script_tail() -> Result {
    let save = Savegame::decode(
        include_bytes!("../../../fixtures/world/populated-extended-v362.sav"),
        ottd_save::DEFAULT_MAX_BYTES,
    )?;
    let mut world = World::decode(&save)?;
    let json = world.saved_json()?;
    let id = json
        .pointer("/chunks/PLYR/records")
        .ok_or("company records")?
        .as_object()
        .ok_or("company records")?
        .iter()
        .find(|(_, r)| r.get("is_ai") == Some(&serde_json::json!(1)))
        .map(|(id, _)| id.parse::<u32>())
        .transpose()?
        .ok_or("AI company")?;
    assert!(
        world
            .edit_field(*b"PLYR", id, &[field("is_ai")], WireValue::Signed(0))
            .is_err()
    );
    Ok(())
}

#[test]
fn rejects_nonexistent_group_sentinels_and_wrong_vehicle_type() -> Result {
    let mut world = populated()?;
    assert!(
        world
            .edit_field(
                *b"VEHS",
                12,
                &vehicle("roadveh", "group_id"),
                WireValue::Unsigned(64_001)
            )
            .is_err()
    );
    assert!(
        world
            .edit_batch(vec![
                edit(
                    *b"VEHS",
                    12,
                    vehicle("roadveh", "group_id"),
                    WireValue::Unsigned(0)
                ),
                edit(
                    *b"GRPS",
                    0,
                    vec![field("vehicle_type")],
                    WireValue::Unsigned(0)
                ),
            ])
            .is_err()
    );
    Ok(())
}
